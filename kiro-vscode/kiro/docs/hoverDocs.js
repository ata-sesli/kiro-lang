'use strict';

const keywordDocs = {
  on: {
    signature: 'on (<condition>) { ... }',
    detail: 'Runs the block when condition is truthy.',
    example: 'on (x > 0) { io.print(x) }'
  },
  off: {
    signature: 'off { ... }',
    detail: 'Optional fallback branch for `on` and loop filters.',
    example: 'on (ok) { ... } off { ... }'
  },
  loop: {
    signature: 'loop on (...) { ... } | loop x in iterable { ... }',
    detail: 'Repeats execution either by condition or by iteration.',
    example: 'loop i in 0..10 per 2 { io.print(i) }'
  },
  in: {
    signature: 'loop x in <iterable>',
    detail: 'Introduces iterator source in `loop`.',
    example: 'loop item in items { io.print(item) }'
  },
  per: {
    signature: 'loop x in range per <step>',
    detail: 'Sets iteration step for range loops.',
    example: 'loop i in 0..10 per 2 { io.print(i) }'
  },
  var: {
    signature: 'var name = <expr>',
    detail: 'Declares a mutable variable binding.',
    example: 'var count = 0'
  },
  fn: {
    signature: 'fn name(args) -> type { ... }',
    detail: 'Declares a function. Return type should be explicit.',
    example: 'fn add(a: num, b: num) -> num { return a + b }'
  },
  pure: {
    signature: 'pure fn ...',
    detail: 'Marks a function as side-effect free with stricter rules.',
    example: 'pure fn square(x: num) -> num { return x * x }'
  },
  return: {
    signature: 'return <expr>',
    detail: 'Returns from the current function.',
    example: 'return x + 1'
  },
  break: {
    signature: 'break',
    detail: 'Exits the nearest loop.',
    example: 'on (done) { break }'
  },
  continue: {
    signature: 'continue',
    detail: 'Skips to the next loop iteration.',
    example: 'on (x == 0) { continue }'
  },
  import: {
    signature: 'import module_name',
    detail: 'Imports another `.kiro` module by name.',
    example: 'import math'
  },
  struct: {
    signature: 'struct Name { field: type ... }',
    detail: 'Defines a structured data type.',
    example: 'struct User { name: str age: num }'
  },
  handle: {
    signature: 'handle Name',
    detail: 'Declares an opaque host-owned value type.',
    example: 'handle Model'
  },
  error: {
    signature: 'error Name = "message"',
    detail: 'Declares a named error value.',
    example: 'error NotFound = "Missing"'
  },
  rust: {
    signature: 'rust fn name(args) -> type[!]',
    detail: 'Declares a host function implemented in Rust glue.',
    example: 'rust fn read(path: str) -> str!'
  },
  run: {
    signature: 'run fn_call(...)',
    detail: 'Starts fire-and-forget asynchronous execution.',
    example: 'run worker(job)'
  },
  rest: {
    signature: 'rest',
    detail: 'Gives other running tasks a chance to continue; does not send data or sleep.',
    example: 'rest'
  },
  check: {
    signature: 'check condition[, "message"]',
    detail: 'Checks that a condition is true; failed checks stop the program with a Kiro diagnostic.',
    example: 'check count > 0, "count must be positive"'
  },
  give: {
    signature: 'give pipe value',
    detail: 'Sends a value into a pipe.',
    example: 'give ch 42'
  },
  take: {
    signature: 'take pipe',
    detail: 'Receives a value from a pipe.',
    example: 'var msg = take ch'
  },
  close: {
    signature: 'close pipe',
    detail: 'Closes a pipe sender.',
    example: 'close ch'
  },
  ref: {
    signature: 'ref expr',
    detail: 'Creates an address/pointer value.',
    example: 'var p = ref x'
  },
  deref: {
    signature: 'deref expr',
    detail: 'Reads value through pointer.',
    example: 'io.print(deref p)'
  },
  move: {
    signature: 'move var',
    detail: 'Moves value out of a mutable variable.',
    example: 'y = move x'
  },
  len: {
    signature: 'len expr',
    detail: 'Returns length of string/list/map.',
    example: 'io.print(len items)'
  },
  push: {
    signature: 'list push value',
    detail: 'Appends value to list.',
    example: 'items push 10'
  },
  at: {
    signature: 'collection at key',
    detail: 'Indexes list/map by key/index.',
    example: 'io.print(nums at 0)'
  },
  num: { signature: 'num', detail: 'Numeric type.', example: 'x: num' },
  str: { signature: 'str', detail: 'String/text type.', example: 'name: str' },
  bool: { signature: 'bool', detail: 'Boolean type.', example: 'ok: bool' },
  void: { signature: 'void', detail: 'No-value type.', example: 'fn log() -> void { ... }' },
  adr: { signature: 'adr <type>', detail: 'Pointer/address type.', example: 'p: adr num' },
  pipe: { signature: 'pipe <type>', detail: 'Channel type.', example: 'ch: pipe str' },
  list: { signature: 'list <type>', detail: 'List collection type.', example: 'xs: list num' },
  map: { signature: 'map <key> <value>', detail: 'Map collection type.', example: 'm: map str num' }
};

const moduleDocs = {
  std_fs: {
    read: {
      signature: 'std_fs.read(path: str) -> str!',
      detail: 'Reads UTF-8 file content.',
      example: 'content = std_fs.read(\"notes.txt\")'
    },
    write: {
      signature: 'std_fs.write(path: str, content: str) -> void!',
      detail: 'Writes text to file (create/truncate).',
      example: 'std_fs.write(\"notes.txt\", \"hello\")'
    },
    exists: {
      signature: 'std_fs.exists(path: str) -> bool',
      detail: 'Checks whether the path exists.',
      example: 'on (std_fs.exists(\"notes.txt\")) { ... }'
    },
    remove: {
      signature: 'std_fs.remove(path: str) -> void!',
      detail: 'Removes a file by path.',
      example: 'std_fs.remove(\"notes.txt\")'
    },
    list: {
      signature: 'std_fs.list(path: str) -> list str!',
      detail: 'Lists directory entries.',
      example: 'files = std_fs.list(\".\")'
    }
  },
  std_env: {
    get: {
      signature: 'std_env.get(key: str) -> str!',
      detail: 'Reads an environment variable.',
      example: 'home = std_env.get(\"HOME\")'
    },
    set: {
      signature: 'std_env.set(key: str, value: str) -> void',
      detail: 'Sets an environment variable.',
      example: 'std_env.set(\"MODE\", \"dev\")'
    },
    args: {
      signature: 'std_env.args() -> list str',
      detail: 'Returns process arguments.',
      example: 'argv = std_env.args()'
    }
  },
  std_net: {
    get: {
      signature: 'std_net.get(url: str) -> str!',
      detail: 'Sends HTTP GET and returns response body text.',
      example: 'body = std_net.get(\"https://example.com\")'
    },
    post: {
      signature: 'std_net.post(url: str, body: str) -> str!',
      detail: 'Sends HTTP POST and returns response body text.',
      example: 'resp = std_net.post(url, payload)'
    },
    status: {
      signature: 'std_net.status(url: str) -> num!',
      detail: 'Returns HTTP status code.',
      example: 'code = std_net.status(url)'
    },
    body: {
      signature: 'std_net.body(response: str) -> str',
      detail: 'Extracts body text from serialized response payload.',
      example: 'text = std_net.body(resp)'
    }
  },
  std_time: {
    now: {
      signature: 'std_time.now() -> num',
      detail: 'Current UNIX timestamp in milliseconds.',
      example: 't = std_time.now()'
    },
    sleep: {
      signature: 'std_time.sleep(ms: num) -> void',
      detail: 'Sleeps asynchronously for given milliseconds.',
      example: 'std_time.sleep(1000)'
    },
    monotonic: {
      signature: 'std_time.monotonic() -> num',
      detail: 'Monotonic timestamp for elapsed-time measurement.',
      example: 'start = std_time.monotonic()'
    }
  },
  io: {
    print: {
      signature: 'io.print(value) -> void',
      detail: 'Writes a displayable Kiro value to stdout with a newline.',
      example: 'io.print("hello")'
    },
    write: {
      signature: 'io.write(value) -> void',
      detail: 'Writes a displayable Kiro value to stdout without a newline.',
      example: 'io.write("loading...")'
    },
    eprint: {
      signature: 'io.eprint(value) -> void',
      detail: 'Writes a displayable Kiro value to stderr without a newline.',
      example: 'io.eprint("debug")'
    },
    eprintline: {
      signature: 'io.eprintline(value) -> void',
      detail: 'Writes a displayable Kiro value to stderr with a newline.',
      example: 'io.eprintline("debug")'
    }
  },
  std_io: {
    print: {
      signature: 'std_io.print(value) -> void',
      detail: 'Compatibility spelling for io.print.',
      example: 'std_io.print("hello")'
    },
    write: {
      signature: 'std_io.write(value) -> void',
      detail: 'Compatibility spelling for io.write.',
      example: 'std_io.write("loading...")'
    },
    eprint: {
      signature: 'std_io.eprint(value) -> void',
      detail: 'Compatibility spelling for io.eprint.',
      example: 'std_io.eprint("debug")'
    },
    eprintline: {
      signature: 'std_io.eprintline(value) -> void',
      detail: 'Compatibility spelling for io.eprintline.',
      example: 'std_io.eprintline("debug")'
    },
    read_line: {
      signature: 'std_io.read_line() -> str!',
      detail: 'Reads one line from stdin (newline-trimmed).',
      example: 'name = std_io.read_line()'
    },
    input: {
      signature: 'std_io.input(prompt: str) -> str!',
      detail: 'Prints prompt and reads one input line.',
      example: 'name = std_io.input("Name: ")'
    },
    input_num: {
      signature: 'std_io.input_num(prompt: str) -> num!',
      detail: 'Prompts and parses numeric input.',
      example: 'age = std_io.input_num("Age: ")'
    },
    input_bool: {
      signature: 'std_io.input_bool(prompt: str) -> bool!',
      detail: 'Prompts and parses boolean input.',
      example: 'ok = std_io.input_bool("Continue? ")'
    },
    parse_num: {
      signature: 'std_io.parse_num(s: str) -> num!',
      detail: 'Parses string into num.',
      example: 'n = std_io.parse_num("42")'
    }
  }
};

module.exports = {
  keywordDocs,
  moduleDocs
};
