use super::*;

impl SemanticCtx<'_> {
    pub(super) fn infer_collection_call(
        &mut self,
        module: &str,
        function: &str,
        func: &grammar::Expression,
        args: &[grammar::Expression],
    ) -> Result<Option<grammar::KiroType>, KiroError> {
        let call_name = format!("{module}.{function}");
        let expected_count = match (module, function) {
            ("std_lists", "join") => 2,
            ("std_lists", "slice") => 3,
            ("std_lists", "reverse") => 1,
            ("std_maps", "has") | ("std_maps", "delete") => 2,
            ("std_maps", "set") => 3,
            _ => return Ok(None),
        };
        if args.len() != expected_count {
            return Err(self.error_at_span(
                ErrorCode::WrongArgumentCount,
                format!(
                    "Wrong argument count for '{}': expected {}, got {}.",
                    call_name,
                    expected_count,
                    args.len()
                ),
                self.required_call_target_span(func),
                "wrong argument count",
            ));
        }

        let inferred = args
            .iter()
            .map(|arg| self.infer_expr(arg))
            .collect::<Result<Vec<_>, _>>()?;
        let require_type = |index: usize| {
            inferred[index].clone().ok_or_else(|| {
                self.error_at_span(
                    ErrorCode::TypeError,
                    format!("Cannot infer argument {} for '{}'.", index + 1, call_name),
                    crate::grammar::expr_span(&args[index]).unwrap_or((0, 0)),
                    "unknown argument type",
                )
            })
        };

        match (module, function) {
            ("std_lists", "join") => {
                let left = require_type(0)?;
                let right = require_type(1)?;
                if !matches!(left, grammar::KiroType::List(_, _)) {
                    return Err(self.error_at_span(
                        ErrorCode::TypeError,
                        format!("lists.join expects lists, got {}.", type_name(&left)),
                        crate::grammar::expr_span(&args[0]).unwrap_or((0, 0)),
                        "wrong collection type",
                    ));
                }
                if !same_type(&left, &right) {
                    return Err(self.error_at_span(
                        ErrorCode::TypeError,
                        "lists.join arguments must have the same list type.",
                        self.required_call_target_span(func),
                        "different list types",
                    ));
                }
                Ok(Some(left))
            }
            ("std_lists", "slice") => {
                let list = require_type(0)?;
                if !matches!(list, grammar::KiroType::List(_, _)) {
                    return Err(self.error_at_span(
                        ErrorCode::TypeError,
                        format!("lists.slice expects a list, got {}.", type_name(&list)),
                        crate::grammar::expr_span(&args[0]).unwrap_or((0, 0)),
                        "wrong collection type",
                    ));
                }
                for index in [1, 2] {
                    let actual = require_type(index)?;
                    if !same_type(&grammar::KiroType::Num, &actual) {
                        return Err(self.error_at_span(
                            ErrorCode::TypeError,
                            format!("lists.slice index must be num, got {}.", type_name(&actual)),
                            crate::grammar::expr_span(&args[index]).unwrap_or((0, 0)),
                            "wrong index type",
                        ));
                    }
                }
                Ok(Some(list))
            }
            ("std_lists", "reverse") => {
                let list = require_type(0)?;
                if !matches!(list, grammar::KiroType::List(_, _)) {
                    return Err(self.error_at_span(
                        ErrorCode::TypeError,
                        format!("lists.reverse expects a list, got {}.", type_name(&list)),
                        crate::grammar::expr_span(&args[0]).unwrap_or((0, 0)),
                        "wrong collection type",
                    ));
                }
                Ok(Some(list))
            }
            ("std_maps", operation) => {
                let map = require_type(0)?;
                let grammar::KiroType::Map(_, key, value) = &map else {
                    return Err(self.error_at_span(
                        ErrorCode::TypeError,
                        format!("maps.{operation} expects a map, got {}.", type_name(&map)),
                        crate::grammar::expr_span(&args[0]).unwrap_or((0, 0)),
                        "wrong collection type",
                    ));
                };
                let actual_key = require_type(1)?;
                if !same_type(key, &actual_key) {
                    return Err(self.error_at_span(
                        ErrorCode::TypeError,
                        format!(
                            "maps.{operation} key must be {}, got {}.",
                            type_name(key),
                            type_name(&actual_key)
                        ),
                        crate::grammar::expr_span(&args[1]).unwrap_or((0, 0)),
                        "wrong key type",
                    ));
                }
                if operation == "set" {
                    let actual_value = require_type(2)?;
                    if !same_type(value, &actual_value) {
                        return Err(self.error_at_span(
                            ErrorCode::TypeError,
                            format!(
                                "maps.set value must be {}, got {}.",
                                type_name(value),
                                type_name(&actual_value)
                            ),
                            crate::grammar::expr_span(&args[2]).unwrap_or((0, 0)),
                            "wrong value type",
                        ));
                    }
                }
                if operation == "has" {
                    Ok(Some(grammar::KiroType::Bool))
                } else {
                    Ok(Some(map))
                }
            }
            _ => unreachable!("known collection operation"),
        }
    }
}
