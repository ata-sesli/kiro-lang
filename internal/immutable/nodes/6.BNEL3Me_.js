import{_ as et,a as tt,b as nt,c as rt,d as st,e as it,f as at,g as lt,h as ot,i as ct,j as pt,k as ut,l as ht}from"../chunks/BY6OO56s.js";import{c as le,d as gt,a as E,f as Z}from"../chunks/D9ipL-bk.js";import"../chunks/3BqX_UDc.js";import{h as Q,D as Ae,F as q,t as pe,a as ze,an as dt,b7 as ft,E as Ce,C as kt,z as mt,aP as bt,aM as xt,al as _t,b8 as wt,b9 as vt,p as yt,ba as G,bb as St,f as Pe,d as Rt,bc as $t,v as V,u as b,$ as Tt,m as W,s as O,ae as U,c as R,r as v,n as At}from"../chunks/JYUbh9Cz.js";import{s as zt,a as Ct}from"../chunks/Cx4QIXuj.js";import{s as oe}from"../chunks/C_RZrMbK.js";import{i as ce}from"../chunks/DzmRTa9X.js";import{e as Pt,i as It}from"../chunks/BzSo38DV.js";import{h as Et}from"../chunks/dR9rAj1E.js";import{i as Lt}from"../chunks/G2BLSHgO.js";import{b as Mt}from"../chunks/dvgM3Hry.js";import{p as Bt}from"../chunks/B7gic_ia.js";function qt(r,e,n=!1,s=!1,t=!1,a=!1){var i=r,o="";if(n){var l=r;Q&&(i=Ae(q(l)))}pe(()=>{var p=dt;if(o===(o=e()??"")){Q&&ze();return}if(n&&!Q){p.nodes=null,l.innerHTML=o,o!==""&&le(q(l),l.lastChild);return}if(p.nodes!==null&&(ft(p.nodes.start,p.nodes.end),p.nodes=null),o!==""){if(Q){Ce.data;for(var c=ze(),u=c;c!==null&&(c.nodeType!==kt||c.data!=="");)u=c,c=mt(c);if(c===null)throw bt(),xt;le(Ce,u),i=Ae(c);return}var h=s?wt:t?vt:void 0,k=_t(s?"svg":t?"math":"template",h);k.innerHTML=o;var d=s||t?k:k.content;if(le(q(d),d.lastChild),s||t)for(;q(d);)i.before(q(d));else i.before(d)}})}function Ot(){return[{slug:"chapter-00"},{slug:"chapter-01"},{slug:"chapter-02"},{slug:"chapter-03"},{slug:"chapter-04"},{slug:"chapter-05"},{slug:"chapter-06"},{slug:"chapter-07"},{slug:"chapter-08"},{slug:"chapter-09"},{slug:"chapter-10"},{slug:"final-project"}]}const rr=Object.freeze(Object.defineProperty({__proto__:null,entries:Ot},Symbol.toStringTag,{value:"Module"})),jt=`// Chapter 1: The Basics

/// A greeting function.
/// Takes a name and prints hello.
fn greet(name: str) {
    print "Hello, " + name + "!"
}

// 1. Basic Printing
print "--- Hello ---"
greet("Learner")

// 2. Variables
print "\\n--- Variables ---"
pi = 3.14 // Immutable (constant)
print "Pi is: " + pi

var count = 0 // Mutable
print "Count: " + count
count = count + 1
print "Count: " + count

// 3. Types
print "\\n--- Types ---"
is_fun = true // bool
message = "Coding: " + is_fun // str + bool
print message

/// A documented struct.
struct User {
    name: str
    active: bool
}
var u = User { name: "Kiro", active: true }
print "User: " + u.name
`,Dt=`// Chapter 2: Control Flow

// 1. Conditionals
print "--- Conditionals ---"
temp = 25
on (temp > 20) {
    print "It's warm."
} off {
    print "It's cold."
}

// 2. Loops
print "\\n--- While Loop ---"
var i = 0
loop on (i < 3) {
    print "i = " + i
    i = i + 1
}

print "\\n--- Range Loop ---"
loop x in 1..4 {
    print "x = " + x
}

print "\\n--- Filter Loop (Odds) ---"
// loop n in 0..10 on (n % 2 != 0) { // Modulo not supported yet
loop n in 1..10 per 2 {
    print "Odd: " + n
}
`,Nt=`// Chapter 3: Functions & Function References

fn shout(msg: str) {
    print msg + "!!!"
}

pure fn multiply(a: num, b: num) -> num {
    return a * b
}

pure fn inc(x: num) -> num {
    return x + 1
}

pure fn dec(x: num) -> num {
    return x - 1
}

pure fn apply(x: num, f: fn(num) -> num) -> num {
    return f(x)
}

pure fn pick(up: bool) -> fn(num) -> num {
    on (up) {
        return ref inc
    } off {
        return ref dec
    }
}

print "--- Functions ---"
shout("Kiro")

print "--- Pure Func ---"
print "3 * 4 = " + multiply(3, 4)

print "--- Function Refs ---"
f = ref inc
print apply(10, f)
g = pick(false)
print g(10)
`,Zt=`// Library Module
pure fn get_msg() -> str {
    return "Hello from Module!"
}
`,Ht=`// Chapter 4: Data Structures

// 1. Structs
struct Item {
    name: str
    price: num
}

var apple = Item { name: "Apple", price: 0.5 }
print "Item: " + apple.name + " Cost: " + apple.price

// 2. Lists
print "\\n--- Lists ---"
var items = list Item {}
items push apple
items push Item { name: "Banana", price: 0.8 }

loop i in items {
    print "- " + i.name
}

// 3. Maps
print "\\n--- Maps ---"
var stock = map str num {
    "Apple" 100,
    "Banana" 50
}
var val = stock at "Apple"
print "Apple Stock: " + val
`,Ft=`// Chapter 5: Error Handling

error TooSmall = "Number is too small"
error TooBig = "Number is too big"

fn analyze(n: num) -> str! {
    on (n < 10) { return TooSmall }
    on (n > 100) { return TooBig }
    return "Just Right"
}

// Test cases
var inputs = list num { 5, 50, 150 }

loop val in inputs {
    var res = analyze(val)
    print "Analyzing " + val + "..."
    
    on (res == TooSmall) {
        print " -> Error: Too Small!"
    } off {
        on (res == TooBig) {
            print " -> Error: Too Big!"
        } off {
            print " -> Result: " + res
        }
    }
}
`,Qt=`// Chapter 6: Advanced Concepts - Example Script

// 1. Pointers
print "--- Pointers ---"
var value = 100
var ptr = ref value

// Dereference into a variable first
var val_from_ptr = deref ptr
print "Value via ptr: " + val_from_ptr

// Mutating via pointer
deref ptr = 200
print "New Value: " + value

// 2. Concurrency with Pipes
print "\\n--- Concurrency ---"

// Create a pipe for strings
var message_pipe = pipe str

// Define worker function
fn worker(out: pipe str) {
    // Simulate work
    // (In a real app, maybe sleep or compute heavy task)
    
    // Send message back
    give out "Task Complete!"
}

// Spawn the worker
run worker(message_pipe)

print "Main thread waiting..."

// Main thread waits for message
var result = take message_pipe
print "Received from worker: " + result

// 3. Simple Host Function Simulation
// (Note: This requires specific Rust runtime support to work properly)

// rust fn read_file(path: str) -> str!
// var file_content = read_file("test.txt")
`,Gt=`// Chapter 6: Async Execution

fn task(id: num) {
    // Simulate some work generally happens here
    print "Task " + id + " started."
}

print "Starting tasks..."

// Spawn multiple tasks
loop i in 1..4 {
    run task(i)
}

print "All tasks spawned. Main exiting."
// Note: In a real app, you'd likely wait for them or use pipes (Chapter 7).
`,Vt=`// Chapter 7: Pipes

// Create a pipe for numbers
var nums = pipe num

// Spawn a producer
fn producer(out: pipe num) {
    loop i in 1..4 {
        print "Sending " + i
        give out i
    }
    // close out // (Optional, if supported language feature)
    print "Producer done."
}

run producer(nums)

// Consume in main
print "Waiting for numbers..."

var total = 0
loop on (total < 6) { // Sum of 1+2+3 = 6
    var n = take nums
    print "Received: " + n
    total = total + n
}

print "Total: " + total
`,Wt=`// Chapter 8: Pointers

var val = 100
var p = ref val

// Reading
var v = deref p
print "Value via ptr: " + v

// Writing
deref p = 200
var updated = deref p
print "Updated via p: " + updated

// Opaque managed handle (adr void)
var any_ptr = adr void
any_ptr = p
print any_ptr

// Struct Pointers
struct Point { x: num y: num }
var pt = Point { x: 10, y: 20 }
var pt_ref = ref pt

// Auto-deref access
print "Point X: " + pt_ref.x
`,Ut=`// Chapter 10: Host Modules
// Note: This script requires 'read_file' to be implemented in Rust runtime.
// It serves as a syntax demonstration.

rust fn read_file(path: str) -> str!

print "Trying to read file via Rust..."

// Mock call (will fail or mock in simulator)
var res = read_file("test.txt")

on (res) {
    print "Content: " + res
} error {
    print "Error reading file (Expected if not implemented in Rust)."
}
`,Kt=`// Final Project: Async Task Manager

struct TaskResult {
    id: num
    success: bool
    message: str
}

// Worker function
fn worker(id: num, out: pipe TaskResult) {
    // Simulate work
    var res = TaskResult {
        id: id,
        success: true,
        message: "Processed task " + id
    }
    
    // Simulate failure for one task
    on (id == 3) {
        res.success = false
        res.message = "Failed task " + id
    }
    
    give out res
}

// Main System
print "System Starting..."
var results_pipe = pipe TaskResult

// Spawn 5 workers
loop i in 1..6 {
    print "Spawning worker " + i
    run worker(i, results_pipe)
}

print "Waiting for results..."

// Collect 5 results
var successes = 0
loop i in 1..6 {
    var r = take results_pipe
    
    on (r.success) {
        print "[OK] " + r.message
        successes = successes + 1
    } off {
        print "[ERR] " + r.message
    }
}

print "System Finished. Success rate: " + successes + "/5"
print "(Note: In 'kiro check' interpreter, rate may be 0/5 due to loop scoping. Run compiled for correct result.)"
`,Xt=`// Glue code for 10_host.kiro
// Note: This is appended to header.rs, so we have access to kiro_runtime types via imports in header.

pub async fn read_file(
    args: Vec<kiro_runtime::RuntimeVal>,
) -> Result<kiro_runtime::RuntimeVal, kiro_runtime::KiroError> {
    // 1. Convert Args
    let path = args
        .get(0)
        .ok_or_else(|| kiro_runtime::KiroError::new("Missing argument"))?
        .as_str()?;

    // 2. Do Work (Mock Implementation for safety/demo)
    // In a real app, we would use tokio::fs::read_to_string(path).await
    // Here we just return a greeting to verify it works.
    let content = format!("Content of {}: Hello from Rust Glue!", path);

    // 3. Return Value
    Ok(kiro_runtime::RuntimeVal::Str(content))
}
`;function de(){return{async:!1,breaks:!1,extensions:null,gfm:!0,hooks:null,pedantic:!1,renderer:null,silent:!1,tokenizer:null,walkTokens:null}}var P=de();function Oe(r){P=r}var z={exec:()=>null};function g(r,e=""){let n=typeof r=="string"?r:r.source,s={replace:(t,a)=>{let i=typeof a=="string"?a:a.source;return i=i.replace(x.caret,"$1"),n=n.replace(t,i),s},getRegex:()=>new RegExp(n,e)};return s}var Jt=(()=>{try{return!!new RegExp("(?<=1)(?<!1)")}catch{return!1}})(),x={codeRemoveIndent:/^(?: {1,4}| {0,3}\t)/gm,outputLinkReplace:/\\([\[\]])/g,indentCodeCompensation:/^(\s+)(?:```)/,beginningSpace:/^\s+/,endingHash:/#$/,startingSpaceChar:/^ /,endingSpaceChar:/ $/,nonSpaceChar:/[^ ]/,newLineCharGlobal:/\n/g,tabCharGlobal:/\t/g,multipleSpaceGlobal:/\s+/g,blankLine:/^[ \t]*$/,doubleBlankLine:/\n[ \t]*\n[ \t]*$/,blockquoteStart:/^ {0,3}>/,blockquoteSetextReplace:/\n {0,3}((?:=+|-+) *)(?=\n|$)/g,blockquoteSetextReplace2:/^ {0,3}>[ \t]?/gm,listReplaceNesting:/^ {1,4}(?=( {4})*[^ ])/g,listIsTask:/^\[[ xX]\] +\S/,listReplaceTask:/^\[[ xX]\] +/,listTaskCheckbox:/\[[ xX]\]/,anyLine:/\n.*\n/,hrefBrackets:/^<(.*)>$/,tableDelimiter:/[:|]/,tableAlignChars:/^\||\| *$/g,tableRowBlankLine:/\n[ \t]*$/,tableAlignRight:/^ *-+: *$/,tableAlignCenter:/^ *:-+: *$/,tableAlignLeft:/^ *:-+ *$/,startATag:/^<a /i,endATag:/^<\/a>/i,startPreScriptTag:/^<(pre|code|kbd|script)(\s|>)/i,endPreScriptTag:/^<\/(pre|code|kbd|script)(\s|>)/i,startAngleBracket:/^</,endAngleBracket:/>$/,pedanticHrefTitle:/^([^'"]*[^\s])\s+(['"])(.*)\2/,unicodeAlphaNumeric:/[\p{L}\p{N}]/u,escapeTest:/[&<>"']/,escapeReplace:/[&<>"']/g,escapeTestNoEncode:/[<>"']|&(?!(#\d{1,7}|#[Xx][a-fA-F0-9]{1,6}|\w+);)/,escapeReplaceNoEncode:/[<>"']|&(?!(#\d{1,7}|#[Xx][a-fA-F0-9]{1,6}|\w+);)/g,caret:/(^|[^\[])\^/g,percentDecode:/%25/g,findPipe:/\|/g,splitPipe:/ \|/,slashPipe:/\\\|/g,carriageReturn:/\r\n|\r/g,spaceLine:/^ +$/gm,notSpaceStart:/^\S*/,endingNewline:/\n$/,listItemRegex:r=>new RegExp(`^( {0,3}${r})((?:[	 ][^\\n]*)?(?:\\n|$))`),nextBulletRegex:r=>new RegExp(`^ {0,${Math.min(3,r-1)}}(?:[*+-]|\\d{1,9}[.)])((?:[ 	][^\\n]*)?(?:\\n|$))`),hrRegex:r=>new RegExp(`^ {0,${Math.min(3,r-1)}}((?:- *){3,}|(?:_ *){3,}|(?:\\* *){3,})(?:\\n+|$)`),fencesBeginRegex:r=>new RegExp(`^ {0,${Math.min(3,r-1)}}(?:\`\`\`|~~~)`),headingBeginRegex:r=>new RegExp(`^ {0,${Math.min(3,r-1)}}#`),htmlBeginRegex:r=>new RegExp(`^ {0,${Math.min(3,r-1)}}<(?:[a-z].*>|!--)`,"i"),blockquoteBeginRegex:r=>new RegExp(`^ {0,${Math.min(3,r-1)}}>`)},Yt=/^(?:[ \t]*(?:\n|$))+/,en=/^((?: {4}| {0,3}\t)[^\n]+(?:\n(?:[ \t]*(?:\n|$))*)?)+/,tn=/^ {0,3}(`{3,}(?=[^`\n]*(?:\n|$))|~{3,})([^\n]*)(?:\n|$)(?:|([\s\S]*?)(?:\n|$))(?: {0,3}\1[~`]* *(?=\n|$)|$)/,H=/^ {0,3}((?:-[\t ]*){3,}|(?:_[ \t]*){3,}|(?:\*[ \t]*){3,})(?:\n+|$)/,nn=/^ {0,3}(#{1,6})(?=\s|$)(.*)(?:\n+|$)/,fe=/ {0,3}(?:[*+-]|\d{1,9}[.)])/,je=/^(?!bull |blockCode|fences|blockquote|heading|html|table)((?:.|\n(?!\s*?\n|bull |blockCode|fences|blockquote|heading|html|table))+?)\n {0,3}(=+|-+) *(?:\n+|$)/,De=g(je).replace(/bull/g,fe).replace(/blockCode/g,/(?: {4}| {0,3}\t)/).replace(/fences/g,/ {0,3}(?:`{3,}|~{3,})/).replace(/blockquote/g,/ {0,3}>/).replace(/heading/g,/ {0,3}#{1,6}/).replace(/html/g,/ {0,3}<[^\n>]+>\n/).replace(/\|table/g,"").getRegex(),rn=g(je).replace(/bull/g,fe).replace(/blockCode/g,/(?: {4}| {0,3}\t)/).replace(/fences/g,/ {0,3}(?:`{3,}|~{3,})/).replace(/blockquote/g,/ {0,3}>/).replace(/heading/g,/ {0,3}#{1,6}/).replace(/html/g,/ {0,3}<[^\n>]+>\n/).replace(/table/g,/ {0,3}\|?(?:[:\- ]*\|)+[\:\- ]*\n/).getRegex(),ke=/^([^\n]+(?:\n(?!hr|heading|lheading|blockquote|fences|list|html|table| +\n)[^\n]+)*)/,sn=/^[^\n]+/,me=/(?!\s*\])(?:\\[\s\S]|[^\[\]\\])+/,an=g(/^ {0,3}\[(label)\]: *(?:\n[ \t]*)?([^<\s][^\s]*|<.*?>)(?:(?: +(?:\n[ \t]*)?| *\n[ \t]*)(title))? *(?:\n+|$)/).replace("label",me).replace("title",/(?:"(?:\\"?|[^"\\])*"|'[^'\n]*(?:\n[^'\n]+)*\n?'|\([^()]*\))/).getRegex(),ln=g(/^(bull)([ \t][^\n]+?)?(?:\n|$)/).replace(/bull/g,fe).getRegex(),ee="address|article|aside|base|basefont|blockquote|body|caption|center|col|colgroup|dd|details|dialog|dir|div|dl|dt|fieldset|figcaption|figure|footer|form|frame|frameset|h[1-6]|head|header|hr|html|iframe|legend|li|link|main|menu|menuitem|meta|nav|noframes|ol|optgroup|option|p|param|search|section|summary|table|tbody|td|tfoot|th|thead|title|tr|track|ul",be=/<!--(?:-?>|[\s\S]*?(?:-->|$))/,on=g("^ {0,3}(?:<(script|pre|style|textarea)[\\s>][\\s\\S]*?(?:</\\1>[^\\n]*\\n+|$)|comment[^\\n]*(\\n+|$)|<\\?[\\s\\S]*?(?:\\?>\\n*|$)|<![A-Z][\\s\\S]*?(?:>\\n*|$)|<!\\[CDATA\\[[\\s\\S]*?(?:\\]\\]>\\n*|$)|</?(tag)(?: +|\\n|/?>)[\\s\\S]*?(?:(?:\\n[ 	]*)+\\n|$)|<(?!script|pre|style|textarea)([a-z][\\w-]*)(?:attribute)*? */?>(?=[ \\t]*(?:\\n|$))[\\s\\S]*?(?:(?:\\n[ 	]*)+\\n|$)|</(?!script|pre|style|textarea)[a-z][\\w-]*\\s*>(?=[ \\t]*(?:\\n|$))[\\s\\S]*?(?:(?:\\n[ 	]*)+\\n|$))","i").replace("comment",be).replace("tag",ee).replace("attribute",/ +[a-zA-Z:_][\w.:-]*(?: *= *"[^"\n]*"| *= *'[^'\n]*'| *= *[^\s"'=<>`]+)?/).getRegex(),Ne=g(ke).replace("hr",H).replace("heading"," {0,3}#{1,6}(?:\\s|$)").replace("|lheading","").replace("|table","").replace("blockquote"," {0,3}>").replace("fences"," {0,3}(?:`{3,}(?=[^`\\n]*\\n)|~{3,})[^\\n]*\\n").replace("list"," {0,3}(?:[*+-]|1[.)])[ \\t]").replace("html","</?(?:tag)(?: +|\\n|/?>)|<(?:script|pre|style|textarea|!--)").replace("tag",ee).getRegex(),cn=g(/^( {0,3}> ?(paragraph|[^\n]*)(?:\n|$))+/).replace("paragraph",Ne).getRegex(),xe={blockquote:cn,code:en,def:an,fences:tn,heading:nn,hr:H,html:on,lheading:De,list:ln,newline:Yt,paragraph:Ne,table:z,text:sn},Ie=g("^ *([^\\n ].*)\\n {0,3}((?:\\| *)?:?-+:? *(?:\\| *:?-+:? *)*(?:\\| *)?)(?:\\n((?:(?! *\\n|hr|heading|blockquote|code|fences|list|html).*(?:\\n|$))*)\\n*|$)").replace("hr",H).replace("heading"," {0,3}#{1,6}(?:\\s|$)").replace("blockquote"," {0,3}>").replace("code","(?: {4}| {0,3}	)[^\\n]").replace("fences"," {0,3}(?:`{3,}(?=[^`\\n]*\\n)|~{3,})[^\\n]*\\n").replace("list"," {0,3}(?:[*+-]|1[.)])[ \\t]").replace("html","</?(?:tag)(?: +|\\n|/?>)|<(?:script|pre|style|textarea|!--)").replace("tag",ee).getRegex(),pn={...xe,lheading:rn,table:Ie,paragraph:g(ke).replace("hr",H).replace("heading"," {0,3}#{1,6}(?:\\s|$)").replace("|lheading","").replace("table",Ie).replace("blockquote"," {0,3}>").replace("fences"," {0,3}(?:`{3,}(?=[^`\\n]*\\n)|~{3,})[^\\n]*\\n").replace("list"," {0,3}(?:[*+-]|1[.)])[ \\t]").replace("html","</?(?:tag)(?: +|\\n|/?>)|<(?:script|pre|style|textarea|!--)").replace("tag",ee).getRegex()},un={...xe,html:g(`^ *(?:comment *(?:\\n|\\s*$)|<(tag)[\\s\\S]+?</\\1> *(?:\\n{2,}|\\s*$)|<tag(?:"[^"]*"|'[^']*'|\\s[^'"/>\\s]*)*?/?> *(?:\\n{2,}|\\s*$))`).replace("comment",be).replace(/tag/g,"(?!(?:a|em|strong|small|s|cite|q|dfn|abbr|data|time|code|var|samp|kbd|sub|sup|i|b|u|mark|ruby|rt|rp|bdi|bdo|span|br|wbr|ins|del|img)\\b)\\w+(?!:|[^\\w\\s@]*@)\\b").getRegex(),def:/^ *\[([^\]]+)\]: *<?([^\s>]+)>?(?: +(["(][^\n]+[")]))? *(?:\n+|$)/,heading:/^(#{1,6})(.*)(?:\n+|$)/,fences:z,lheading:/^(.+?)\n {0,3}(=+|-+) *(?:\n+|$)/,paragraph:g(ke).replace("hr",H).replace("heading",` *#{1,6} *[^
]`).replace("lheading",De).replace("|table","").replace("blockquote"," {0,3}>").replace("|fences","").replace("|list","").replace("|html","").replace("|tag","").getRegex()},hn=/^\\([!"#$%&'()*+,\-./:;<=>?@\[\]\\^_`{|}~])/,gn=/^(`+)([^`]|[^`][\s\S]*?[^`])\1(?!`)/,Ze=/^( {2,}|\\)\n(?!\s*$)/,dn=/^(`+|[^`])(?:(?= {2,}\n)|[\s\S]*?(?:(?=[\\<!\[`*_]|\b_|$)|[^ ](?= {2,}\n)))/,L=/[\p{P}\p{S}]/u,te=/[\s\p{P}\p{S}]/u,_e=/[^\s\p{P}\p{S}]/u,fn=g(/^((?![*_])punctSpace)/,"u").replace(/punctSpace/g,te).getRegex(),He=/(?!~)[\p{P}\p{S}]/u,kn=/(?!~)[\s\p{P}\p{S}]/u,mn=/(?:[^\s\p{P}\p{S}]|~)/u,bn=g(/link|precode-code|html/,"g").replace("link",/\[(?:[^\[\]`]|(?<a>`+)[^`]+\k<a>(?!`))*?\]\((?:\\[\s\S]|[^\\\(\)]|\((?:\\[\s\S]|[^\\\(\)])*\))*\)/).replace("precode-",Jt?"(?<!`)()":"(^^|[^`])").replace("code",/(?<b>`+)[^`]+\k<b>(?!`)/).replace("html",/<(?! )[^<>]*?>/).getRegex(),Fe=/^(?:\*+(?:((?!\*)punct)|([^\s*]))?)|^_+(?:((?!_)punct)|([^\s_]))?/,xn=g(Fe,"u").replace(/punct/g,L).getRegex(),_n=g(Fe,"u").replace(/punct/g,He).getRegex(),Qe="^[^_*]*?__[^_*]*?\\*[^_*]*?(?=__)|[^*]+(?=[^*])|(?!\\*)punct(\\*+)(?=[\\s]|$)|notPunctSpace(\\*+)(?!\\*)(?=punctSpace|$)|(?!\\*)punctSpace(\\*+)(?=notPunctSpace)|[\\s](\\*+)(?!\\*)(?=punct)|(?!\\*)punct(\\*+)(?!\\*)(?=punct)|notPunctSpace(\\*+)(?=notPunctSpace)",wn=g(Qe,"gu").replace(/notPunctSpace/g,_e).replace(/punctSpace/g,te).replace(/punct/g,L).getRegex(),vn=g(Qe,"gu").replace(/notPunctSpace/g,mn).replace(/punctSpace/g,kn).replace(/punct/g,He).getRegex(),yn=g("^[^_*]*?\\*\\*[^_*]*?_[^_*]*?(?=\\*\\*)|[^_]+(?=[^_])|(?!_)punct(_+)(?=[\\s]|$)|notPunctSpace(_+)(?!_)(?=punctSpace|$)|(?!_)punctSpace(_+)(?=notPunctSpace)|[\\s](_+)(?!_)(?=punct)|(?!_)punct(_+)(?!_)(?=punct)","gu").replace(/notPunctSpace/g,_e).replace(/punctSpace/g,te).replace(/punct/g,L).getRegex(),Sn=g(/^~~?(?:((?!~)punct)|[^\s~])/,"u").replace(/punct/g,L).getRegex(),Rn="^[^~]+(?=[^~])|(?!~)punct(~~?)(?=[\\s]|$)|notPunctSpace(~~?)(?!~)(?=punctSpace|$)|(?!~)punctSpace(~~?)(?=notPunctSpace)|[\\s](~~?)(?!~)(?=punct)|(?!~)punct(~~?)(?!~)(?=punct)|notPunctSpace(~~?)(?=notPunctSpace)",$n=g(Rn,"gu").replace(/notPunctSpace/g,_e).replace(/punctSpace/g,te).replace(/punct/g,L).getRegex(),Tn=g(/\\(punct)/,"gu").replace(/punct/g,L).getRegex(),An=g(/^<(scheme:[^\s\x00-\x1f<>]*|email)>/).replace("scheme",/[a-zA-Z][a-zA-Z0-9+.-]{1,31}/).replace("email",/[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+(@)[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)+(?![-_])/).getRegex(),zn=g(be).replace("(?:-->|$)","-->").getRegex(),Cn=g("^comment|^</[a-zA-Z][\\w:-]*\\s*>|^<[a-zA-Z][\\w-]*(?:attribute)*?\\s*/?>|^<\\?[\\s\\S]*?\\?>|^<![a-zA-Z]+\\s[\\s\\S]*?>|^<!\\[CDATA\\[[\\s\\S]*?\\]\\]>").replace("comment",zn).replace("attribute",/\s+[a-zA-Z:_][\w.:-]*(?:\s*=\s*"[^"]*"|\s*=\s*'[^']*'|\s*=\s*[^\s"'=<>`]+)?/).getRegex(),X=/(?:\[(?:\\[\s\S]|[^\[\]\\])*\]|\\[\s\S]|`+(?!`)[^`]*?`+(?!`)|``+(?=\])|[^\[\]\\`])*?/,Pn=g(/^!?\[(label)\]\(\s*(href)(?:(?:[ \t]+(?:\n[ \t]*)?|\n[ \t]*)(title))?\s*\)/).replace("label",X).replace("href",/<(?:\\.|[^\n<>\\])+>|[^ \t\n\x00-\x1f]*/).replace("title",/"(?:\\"?|[^"\\])*"|'(?:\\'?|[^'\\])*'|\((?:\\\)?|[^)\\])*\)/).getRegex(),Ge=g(/^!?\[(label)\]\[(ref)\]/).replace("label",X).replace("ref",me).getRegex(),Ve=g(/^!?\[(ref)\](?:\[\])?/).replace("ref",me).getRegex(),In=g("reflink|nolink(?!\\()","g").replace("reflink",Ge).replace("nolink",Ve).getRegex(),Ee=/[hH][tT][tT][pP][sS]?|[fF][tT][pP]/,we={_backpedal:z,anyPunctuation:Tn,autolink:An,blockSkip:bn,br:Ze,code:gn,del:z,delLDelim:z,delRDelim:z,emStrongLDelim:xn,emStrongRDelimAst:wn,emStrongRDelimUnd:yn,escape:hn,link:Pn,nolink:Ve,punctuation:fn,reflink:Ge,reflinkSearch:In,tag:Cn,text:dn,url:z},En={...we,link:g(/^!?\[(label)\]\((.*?)\)/).replace("label",X).getRegex(),reflink:g(/^!?\[(label)\]\s*\[([^\]]*)\]/).replace("label",X).getRegex()},ue={...we,emStrongRDelimAst:vn,emStrongLDelim:_n,delLDelim:Sn,delRDelim:$n,url:g(/^((?:protocol):\/\/|www\.)(?:[a-zA-Z0-9\-]+\.?)+[^\s<]*|^email/).replace("protocol",Ee).replace("email",/[A-Za-z0-9._+-]+(@)[a-zA-Z0-9-_]+(?:\.[a-zA-Z0-9-_]*[a-zA-Z0-9])+(?![-_])/).getRegex(),_backpedal:/(?:[^?!.,:;*_'"~()&]+|\([^)]*\)|&(?![a-zA-Z0-9]+;$)|[?!.,:;*_'"~)]+(?!$))+/,del:/^(~~?)(?=[^\s~])((?:\\[\s\S]|[^\\])*?(?:\\[\s\S]|[^\s~\\]))\1(?=[^~]|$)/,text:g(/^([`~]+|[^`~])(?:(?= {2,}\n)|(?=[a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-]+@)|[\s\S]*?(?:(?=[\\<!\[`*~_]|\b_|protocol:\/\/|www\.|$)|[^ ](?= {2,}\n)|[^a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-](?=[a-zA-Z0-9.!#$%&'*+\/=?_`{\|}~-]+@)))/).replace("protocol",Ee).getRegex()},Ln={...ue,br:g(Ze).replace("{2,}","*").getRegex(),text:g(ue.text).replace("\\b_","\\b_| {2,}\\n").replace(/\{2,\}/g,"*").getRegex()},K={normal:xe,gfm:pn,pedantic:un},j={normal:we,gfm:ue,breaks:Ln,pedantic:En},Mn={"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"},Le=r=>Mn[r];function $(r,e){if(e){if(x.escapeTest.test(r))return r.replace(x.escapeReplace,Le)}else if(x.escapeTestNoEncode.test(r))return r.replace(x.escapeReplaceNoEncode,Le);return r}function Me(r){try{r=encodeURI(r).replace(x.percentDecode,"%")}catch{return null}return r}function Be(r,e){let n=r.replace(x.findPipe,(a,i,o)=>{let l=!1,p=i;for(;--p>=0&&o[p]==="\\";)l=!l;return l?"|":" |"}),s=n.split(x.splitPipe),t=0;if(s[0].trim()||s.shift(),s.length>0&&!s.at(-1)?.trim()&&s.pop(),e)if(s.length>e)s.splice(e);else for(;s.length<e;)s.push("");for(;t<s.length;t++)s[t]=s[t].trim().replace(x.slashPipe,"|");return s}function D(r,e,n){let s=r.length;if(s===0)return"";let t=0;for(;t<s&&r.charAt(s-t-1)===e;)t++;return r.slice(0,s-t)}function Bn(r,e){if(r.indexOf(e[1])===-1)return-1;let n=0;for(let s=0;s<r.length;s++)if(r[s]==="\\")s++;else if(r[s]===e[0])n++;else if(r[s]===e[1]&&(n--,n<0))return s;return n>0?-2:-1}function qn(r,e=0){let n=e,s="";for(let t of r)if(t==="	"){let a=4-n%4;s+=" ".repeat(a),n+=a}else s+=t,n++;return s}function qe(r,e,n,s,t){let a=e.href,i=e.title||null,o=r[1].replace(t.other.outputLinkReplace,"$1");s.state.inLink=!0;let l={type:r[0].charAt(0)==="!"?"image":"link",raw:n,href:a,title:i,text:o,tokens:s.inlineTokens(o)};return s.state.inLink=!1,l}function On(r,e,n){let s=r.match(n.other.indentCodeCompensation);if(s===null)return e;let t=s[1];return e.split(`
`).map(a=>{let i=a.match(n.other.beginningSpace);if(i===null)return a;let[o]=i;return o.length>=t.length?a.slice(t.length):a}).join(`
`)}var J=class{options;rules;lexer;constructor(r){this.options=r||P}space(r){let e=this.rules.block.newline.exec(r);if(e&&e[0].length>0)return{type:"space",raw:e[0]}}code(r){let e=this.rules.block.code.exec(r);if(e){let n=e[0].replace(this.rules.other.codeRemoveIndent,"");return{type:"code",raw:e[0],codeBlockStyle:"indented",text:this.options.pedantic?n:D(n,`
`)}}}fences(r){let e=this.rules.block.fences.exec(r);if(e){let n=e[0],s=On(n,e[3]||"",this.rules);return{type:"code",raw:n,lang:e[2]?e[2].trim().replace(this.rules.inline.anyPunctuation,"$1"):e[2],text:s}}}heading(r){let e=this.rules.block.heading.exec(r);if(e){let n=e[2].trim();if(this.rules.other.endingHash.test(n)){let s=D(n,"#");(this.options.pedantic||!s||this.rules.other.endingSpaceChar.test(s))&&(n=s.trim())}return{type:"heading",raw:e[0],depth:e[1].length,text:n,tokens:this.lexer.inline(n)}}}hr(r){let e=this.rules.block.hr.exec(r);if(e)return{type:"hr",raw:D(e[0],`
`)}}blockquote(r){let e=this.rules.block.blockquote.exec(r);if(e){let n=D(e[0],`
`).split(`
`),s="",t="",a=[];for(;n.length>0;){let i=!1,o=[],l;for(l=0;l<n.length;l++)if(this.rules.other.blockquoteStart.test(n[l]))o.push(n[l]),i=!0;else if(!i)o.push(n[l]);else break;n=n.slice(l);let p=o.join(`
`),c=p.replace(this.rules.other.blockquoteSetextReplace,`
    $1`).replace(this.rules.other.blockquoteSetextReplace2,"");s=s?`${s}
${p}`:p,t=t?`${t}
${c}`:c;let u=this.lexer.state.top;if(this.lexer.state.top=!0,this.lexer.blockTokens(c,a,!0),this.lexer.state.top=u,n.length===0)break;let h=a.at(-1);if(h?.type==="code")break;if(h?.type==="blockquote"){let k=h,d=k.raw+`
`+n.join(`
`),w=this.blockquote(d);a[a.length-1]=w,s=s.substring(0,s.length-k.raw.length)+w.raw,t=t.substring(0,t.length-k.text.length)+w.text;break}else if(h?.type==="list"){let k=h,d=k.raw+`
`+n.join(`
`),w=this.list(d);a[a.length-1]=w,s=s.substring(0,s.length-h.raw.length)+w.raw,t=t.substring(0,t.length-k.raw.length)+w.raw,n=d.substring(a.at(-1).raw.length).split(`
`);continue}}return{type:"blockquote",raw:s,tokens:a,text:t}}}list(r){let e=this.rules.block.list.exec(r);if(e){let n=e[1].trim(),s=n.length>1,t={type:"list",raw:"",ordered:s,start:s?+n.slice(0,-1):"",loose:!1,items:[]};n=s?`\\d{1,9}\\${n.slice(-1)}`:`\\${n}`,this.options.pedantic&&(n=s?n:"[*+-]");let a=this.rules.other.listItemRegex(n),i=!1;for(;r;){let l=!1,p="",c="";if(!(e=a.exec(r))||this.rules.block.hr.test(r))break;p=e[0],r=r.substring(p.length);let u=qn(e[2].split(`
`,1)[0],e[1].length),h=r.split(`
`,1)[0],k=!u.trim(),d=0;if(this.options.pedantic?(d=2,c=u.trimStart()):k?d=e[1].length+1:(d=u.search(this.rules.other.nonSpaceChar),d=d>4?1:d,c=u.slice(d),d+=e[1].length),k&&this.rules.other.blankLine.test(h)&&(p+=h+`
`,r=r.substring(h.length+1),l=!0),!l){let w=this.rules.other.nextBulletRegex(d),F=this.rules.other.hrRegex(d),m=this.rules.other.fencesBeginRegex(d),_=this.rules.other.headingBeginRegex(d),A=this.rules.other.htmlBeginRegex(d),M=this.rules.other.blockquoteBeginRegex(d);for(;r;){let B=r.split(`
`,1)[0],T;if(h=B,this.options.pedantic?(h=h.replace(this.rules.other.listReplaceNesting,"  "),T=h):T=h.replace(this.rules.other.tabCharGlobal,"    "),m.test(h)||_.test(h)||A.test(h)||M.test(h)||w.test(h)||F.test(h))break;if(T.search(this.rules.other.nonSpaceChar)>=d||!h.trim())c+=`
`+T.slice(d);else{if(k||u.replace(this.rules.other.tabCharGlobal,"    ").search(this.rules.other.nonSpaceChar)>=4||m.test(u)||_.test(u)||F.test(u))break;c+=`
`+h}k=!h.trim(),p+=B+`
`,r=r.substring(B.length+1),u=T.slice(d)}}t.loose||(i?t.loose=!0:this.rules.other.doubleBlankLine.test(p)&&(i=!0)),t.items.push({type:"list_item",raw:p,task:!!this.options.gfm&&this.rules.other.listIsTask.test(c),loose:!1,text:c,tokens:[]}),t.raw+=p}let o=t.items.at(-1);if(o)o.raw=o.raw.trimEnd(),o.text=o.text.trimEnd();else return;t.raw=t.raw.trimEnd();for(let l of t.items){if(this.lexer.state.top=!1,l.tokens=this.lexer.blockTokens(l.text,[]),l.task){if(l.text=l.text.replace(this.rules.other.listReplaceTask,""),l.tokens[0]?.type==="text"||l.tokens[0]?.type==="paragraph"){l.tokens[0].raw=l.tokens[0].raw.replace(this.rules.other.listReplaceTask,""),l.tokens[0].text=l.tokens[0].text.replace(this.rules.other.listReplaceTask,"");for(let c=this.lexer.inlineQueue.length-1;c>=0;c--)if(this.rules.other.listIsTask.test(this.lexer.inlineQueue[c].src)){this.lexer.inlineQueue[c].src=this.lexer.inlineQueue[c].src.replace(this.rules.other.listReplaceTask,"");break}}let p=this.rules.other.listTaskCheckbox.exec(l.raw);if(p){let c={type:"checkbox",raw:p[0]+" ",checked:p[0]!=="[ ]"};l.checked=c.checked,t.loose?l.tokens[0]&&["paragraph","text"].includes(l.tokens[0].type)&&"tokens"in l.tokens[0]&&l.tokens[0].tokens?(l.tokens[0].raw=c.raw+l.tokens[0].raw,l.tokens[0].text=c.raw+l.tokens[0].text,l.tokens[0].tokens.unshift(c)):l.tokens.unshift({type:"paragraph",raw:c.raw,text:c.raw,tokens:[c]}):l.tokens.unshift(c)}}if(!t.loose){let p=l.tokens.filter(u=>u.type==="space"),c=p.length>0&&p.some(u=>this.rules.other.anyLine.test(u.raw));t.loose=c}}if(t.loose)for(let l of t.items){l.loose=!0;for(let p of l.tokens)p.type==="text"&&(p.type="paragraph")}return t}}html(r){let e=this.rules.block.html.exec(r);if(e)return{type:"html",block:!0,raw:e[0],pre:e[1]==="pre"||e[1]==="script"||e[1]==="style",text:e[0]}}def(r){let e=this.rules.block.def.exec(r);if(e){let n=e[1].toLowerCase().replace(this.rules.other.multipleSpaceGlobal," "),s=e[2]?e[2].replace(this.rules.other.hrefBrackets,"$1").replace(this.rules.inline.anyPunctuation,"$1"):"",t=e[3]?e[3].substring(1,e[3].length-1).replace(this.rules.inline.anyPunctuation,"$1"):e[3];return{type:"def",tag:n,raw:e[0],href:s,title:t}}}table(r){let e=this.rules.block.table.exec(r);if(!e||!this.rules.other.tableDelimiter.test(e[2]))return;let n=Be(e[1]),s=e[2].replace(this.rules.other.tableAlignChars,"").split("|"),t=e[3]?.trim()?e[3].replace(this.rules.other.tableRowBlankLine,"").split(`
`):[],a={type:"table",raw:e[0],header:[],align:[],rows:[]};if(n.length===s.length){for(let i of s)this.rules.other.tableAlignRight.test(i)?a.align.push("right"):this.rules.other.tableAlignCenter.test(i)?a.align.push("center"):this.rules.other.tableAlignLeft.test(i)?a.align.push("left"):a.align.push(null);for(let i=0;i<n.length;i++)a.header.push({text:n[i],tokens:this.lexer.inline(n[i]),header:!0,align:a.align[i]});for(let i of t)a.rows.push(Be(i,a.header.length).map((o,l)=>({text:o,tokens:this.lexer.inline(o),header:!1,align:a.align[l]})));return a}}lheading(r){let e=this.rules.block.lheading.exec(r);if(e){let n=e[1].trim();return{type:"heading",raw:e[0],depth:e[2].charAt(0)==="="?1:2,text:n,tokens:this.lexer.inline(n)}}}paragraph(r){let e=this.rules.block.paragraph.exec(r);if(e){let n=e[1].charAt(e[1].length-1)===`
`?e[1].slice(0,-1):e[1];return{type:"paragraph",raw:e[0],text:n,tokens:this.lexer.inline(n)}}}text(r){let e=this.rules.block.text.exec(r);if(e)return{type:"text",raw:e[0],text:e[0],tokens:this.lexer.inline(e[0])}}escape(r){let e=this.rules.inline.escape.exec(r);if(e)return{type:"escape",raw:e[0],text:e[1]}}tag(r){let e=this.rules.inline.tag.exec(r);if(e)return!this.lexer.state.inLink&&this.rules.other.startATag.test(e[0])?this.lexer.state.inLink=!0:this.lexer.state.inLink&&this.rules.other.endATag.test(e[0])&&(this.lexer.state.inLink=!1),!this.lexer.state.inRawBlock&&this.rules.other.startPreScriptTag.test(e[0])?this.lexer.state.inRawBlock=!0:this.lexer.state.inRawBlock&&this.rules.other.endPreScriptTag.test(e[0])&&(this.lexer.state.inRawBlock=!1),{type:"html",raw:e[0],inLink:this.lexer.state.inLink,inRawBlock:this.lexer.state.inRawBlock,block:!1,text:e[0]}}link(r){let e=this.rules.inline.link.exec(r);if(e){let n=e[2].trim();if(!this.options.pedantic&&this.rules.other.startAngleBracket.test(n)){if(!this.rules.other.endAngleBracket.test(n))return;let a=D(n.slice(0,-1),"\\");if((n.length-a.length)%2===0)return}else{let a=Bn(e[2],"()");if(a===-2)return;if(a>-1){let i=(e[0].indexOf("!")===0?5:4)+e[1].length+a;e[2]=e[2].substring(0,a),e[0]=e[0].substring(0,i).trim(),e[3]=""}}let s=e[2],t="";if(this.options.pedantic){let a=this.rules.other.pedanticHrefTitle.exec(s);a&&(s=a[1],t=a[3])}else t=e[3]?e[3].slice(1,-1):"";return s=s.trim(),this.rules.other.startAngleBracket.test(s)&&(this.options.pedantic&&!this.rules.other.endAngleBracket.test(n)?s=s.slice(1):s=s.slice(1,-1)),qe(e,{href:s&&s.replace(this.rules.inline.anyPunctuation,"$1"),title:t&&t.replace(this.rules.inline.anyPunctuation,"$1")},e[0],this.lexer,this.rules)}}reflink(r,e){let n;if((n=this.rules.inline.reflink.exec(r))||(n=this.rules.inline.nolink.exec(r))){let s=(n[2]||n[1]).replace(this.rules.other.multipleSpaceGlobal," "),t=e[s.toLowerCase()];if(!t){let a=n[0].charAt(0);return{type:"text",raw:a,text:a}}return qe(n,t,n[0],this.lexer,this.rules)}}emStrong(r,e,n=""){let s=this.rules.inline.emStrongLDelim.exec(r);if(!(!s||!s[1]&&!s[2]&&!s[3]&&!s[4]||s[4]&&n.match(this.rules.other.unicodeAlphaNumeric))&&(!(s[1]||s[3])||!n||this.rules.inline.punctuation.exec(n))){let t=[...s[0]].length-1,a,i,o=t,l=0,p=s[0][0]==="*"?this.rules.inline.emStrongRDelimAst:this.rules.inline.emStrongRDelimUnd;for(p.lastIndex=0,e=e.slice(-1*r.length+t);(s=p.exec(e))!==null;){if(a=s[1]||s[2]||s[3]||s[4]||s[5]||s[6],!a)continue;if(i=[...a].length,s[3]||s[4]){o+=i;continue}else if((s[5]||s[6])&&t%3&&!((t+i)%3)){l+=i;continue}if(o-=i,o>0)continue;i=Math.min(i,i+o+l);let c=[...s[0]][0].length,u=r.slice(0,t+s.index+c+i);if(Math.min(t,i)%2){let k=u.slice(1,-1);return{type:"em",raw:u,text:k,tokens:this.lexer.inlineTokens(k)}}let h=u.slice(2,-2);return{type:"strong",raw:u,text:h,tokens:this.lexer.inlineTokens(h)}}}}codespan(r){let e=this.rules.inline.code.exec(r);if(e){let n=e[2].replace(this.rules.other.newLineCharGlobal," "),s=this.rules.other.nonSpaceChar.test(n),t=this.rules.other.startingSpaceChar.test(n)&&this.rules.other.endingSpaceChar.test(n);return s&&t&&(n=n.substring(1,n.length-1)),{type:"codespan",raw:e[0],text:n}}}br(r){let e=this.rules.inline.br.exec(r);if(e)return{type:"br",raw:e[0]}}del(r,e,n=""){let s=this.rules.inline.delLDelim.exec(r);if(s&&(!s[1]||!n||this.rules.inline.punctuation.exec(n))){let t=[...s[0]].length-1,a,i,o=t,l=this.rules.inline.delRDelim;for(l.lastIndex=0,e=e.slice(-1*r.length+t);(s=l.exec(e))!==null;){if(a=s[1]||s[2]||s[3]||s[4]||s[5]||s[6],!a||(i=[...a].length,i!==t))continue;if(s[3]||s[4]){o+=i;continue}if(o-=i,o>0)continue;i=Math.min(i,i+o);let p=[...s[0]][0].length,c=r.slice(0,t+s.index+p+i),u=c.slice(t,-t);return{type:"del",raw:c,text:u,tokens:this.lexer.inlineTokens(u)}}}}autolink(r){let e=this.rules.inline.autolink.exec(r);if(e){let n,s;return e[2]==="@"?(n=e[1],s="mailto:"+n):(n=e[1],s=n),{type:"link",raw:e[0],text:n,href:s,tokens:[{type:"text",raw:n,text:n}]}}}url(r){let e;if(e=this.rules.inline.url.exec(r)){let n,s;if(e[2]==="@")n=e[0],s="mailto:"+n;else{let t;do t=e[0],e[0]=this.rules.inline._backpedal.exec(e[0])?.[0]??"";while(t!==e[0]);n=e[0],e[1]==="www."?s="http://"+e[0]:s=e[0]}return{type:"link",raw:e[0],text:n,href:s,tokens:[{type:"text",raw:n,text:n}]}}}inlineText(r){let e=this.rules.inline.text.exec(r);if(e){let n=this.lexer.state.inRawBlock;return{type:"text",raw:e[0],text:e[0],escaped:n}}}},y=class he{tokens;options;state;inlineQueue;tokenizer;constructor(e){this.tokens=[],this.tokens.links=Object.create(null),this.options=e||P,this.options.tokenizer=this.options.tokenizer||new J,this.tokenizer=this.options.tokenizer,this.tokenizer.options=this.options,this.tokenizer.lexer=this,this.inlineQueue=[],this.state={inLink:!1,inRawBlock:!1,top:!0};let n={other:x,block:K.normal,inline:j.normal};this.options.pedantic?(n.block=K.pedantic,n.inline=j.pedantic):this.options.gfm&&(n.block=K.gfm,this.options.breaks?n.inline=j.breaks:n.inline=j.gfm),this.tokenizer.rules=n}static get rules(){return{block:K,inline:j}}static lex(e,n){return new he(n).lex(e)}static lexInline(e,n){return new he(n).inlineTokens(e)}lex(e){e=e.replace(x.carriageReturn,`
`),this.blockTokens(e,this.tokens);for(let n=0;n<this.inlineQueue.length;n++){let s=this.inlineQueue[n];this.inlineTokens(s.src,s.tokens)}return this.inlineQueue=[],this.tokens}blockTokens(e,n=[],s=!1){for(this.tokenizer.lexer=this,this.options.pedantic&&(e=e.replace(x.tabCharGlobal,"    ").replace(x.spaceLine,""));e;){let t;if(this.options.extensions?.block?.some(i=>(t=i.call({lexer:this},e,n))?(e=e.substring(t.raw.length),n.push(t),!0):!1))continue;if(t=this.tokenizer.space(e)){e=e.substring(t.raw.length);let i=n.at(-1);t.raw.length===1&&i!==void 0?i.raw+=`
`:n.push(t);continue}if(t=this.tokenizer.code(e)){e=e.substring(t.raw.length);let i=n.at(-1);i?.type==="paragraph"||i?.type==="text"?(i.raw+=(i.raw.endsWith(`
`)?"":`
`)+t.raw,i.text+=`
`+t.text,this.inlineQueue.at(-1).src=i.text):n.push(t);continue}if(t=this.tokenizer.fences(e)){e=e.substring(t.raw.length),n.push(t);continue}if(t=this.tokenizer.heading(e)){e=e.substring(t.raw.length),n.push(t);continue}if(t=this.tokenizer.hr(e)){e=e.substring(t.raw.length),n.push(t);continue}if(t=this.tokenizer.blockquote(e)){e=e.substring(t.raw.length),n.push(t);continue}if(t=this.tokenizer.list(e)){e=e.substring(t.raw.length),n.push(t);continue}if(t=this.tokenizer.html(e)){e=e.substring(t.raw.length),n.push(t);continue}if(t=this.tokenizer.def(e)){e=e.substring(t.raw.length);let i=n.at(-1);i?.type==="paragraph"||i?.type==="text"?(i.raw+=(i.raw.endsWith(`
`)?"":`
`)+t.raw,i.text+=`
`+t.raw,this.inlineQueue.at(-1).src=i.text):this.tokens.links[t.tag]||(this.tokens.links[t.tag]={href:t.href,title:t.title},n.push(t));continue}if(t=this.tokenizer.table(e)){e=e.substring(t.raw.length),n.push(t);continue}if(t=this.tokenizer.lheading(e)){e=e.substring(t.raw.length),n.push(t);continue}let a=e;if(this.options.extensions?.startBlock){let i=1/0,o=e.slice(1),l;this.options.extensions.startBlock.forEach(p=>{l=p.call({lexer:this},o),typeof l=="number"&&l>=0&&(i=Math.min(i,l))}),i<1/0&&i>=0&&(a=e.substring(0,i+1))}if(this.state.top&&(t=this.tokenizer.paragraph(a))){let i=n.at(-1);s&&i?.type==="paragraph"?(i.raw+=(i.raw.endsWith(`
`)?"":`
`)+t.raw,i.text+=`
`+t.text,this.inlineQueue.pop(),this.inlineQueue.at(-1).src=i.text):n.push(t),s=a.length!==e.length,e=e.substring(t.raw.length);continue}if(t=this.tokenizer.text(e)){e=e.substring(t.raw.length);let i=n.at(-1);i?.type==="text"?(i.raw+=(i.raw.endsWith(`
`)?"":`
`)+t.raw,i.text+=`
`+t.text,this.inlineQueue.pop(),this.inlineQueue.at(-1).src=i.text):n.push(t);continue}if(e){let i="Infinite loop on byte: "+e.charCodeAt(0);if(this.options.silent){console.error(i);break}else throw new Error(i)}}return this.state.top=!0,n}inline(e,n=[]){return this.inlineQueue.push({src:e,tokens:n}),n}inlineTokens(e,n=[]){this.tokenizer.lexer=this;let s=e,t=null;if(this.tokens.links){let l=Object.keys(this.tokens.links);if(l.length>0)for(;(t=this.tokenizer.rules.inline.reflinkSearch.exec(s))!==null;)l.includes(t[0].slice(t[0].lastIndexOf("[")+1,-1))&&(s=s.slice(0,t.index)+"["+"a".repeat(t[0].length-2)+"]"+s.slice(this.tokenizer.rules.inline.reflinkSearch.lastIndex))}for(;(t=this.tokenizer.rules.inline.anyPunctuation.exec(s))!==null;)s=s.slice(0,t.index)+"++"+s.slice(this.tokenizer.rules.inline.anyPunctuation.lastIndex);let a;for(;(t=this.tokenizer.rules.inline.blockSkip.exec(s))!==null;)a=t[2]?t[2].length:0,s=s.slice(0,t.index+a)+"["+"a".repeat(t[0].length-a-2)+"]"+s.slice(this.tokenizer.rules.inline.blockSkip.lastIndex);s=this.options.hooks?.emStrongMask?.call({lexer:this},s)??s;let i=!1,o="";for(;e;){i||(o=""),i=!1;let l;if(this.options.extensions?.inline?.some(c=>(l=c.call({lexer:this},e,n))?(e=e.substring(l.raw.length),n.push(l),!0):!1))continue;if(l=this.tokenizer.escape(e)){e=e.substring(l.raw.length),n.push(l);continue}if(l=this.tokenizer.tag(e)){e=e.substring(l.raw.length),n.push(l);continue}if(l=this.tokenizer.link(e)){e=e.substring(l.raw.length),n.push(l);continue}if(l=this.tokenizer.reflink(e,this.tokens.links)){e=e.substring(l.raw.length);let c=n.at(-1);l.type==="text"&&c?.type==="text"?(c.raw+=l.raw,c.text+=l.text):n.push(l);continue}if(l=this.tokenizer.emStrong(e,s,o)){e=e.substring(l.raw.length),n.push(l);continue}if(l=this.tokenizer.codespan(e)){e=e.substring(l.raw.length),n.push(l);continue}if(l=this.tokenizer.br(e)){e=e.substring(l.raw.length),n.push(l);continue}if(l=this.tokenizer.del(e,s,o)){e=e.substring(l.raw.length),n.push(l);continue}if(l=this.tokenizer.autolink(e)){e=e.substring(l.raw.length),n.push(l);continue}if(!this.state.inLink&&(l=this.tokenizer.url(e))){e=e.substring(l.raw.length),n.push(l);continue}let p=e;if(this.options.extensions?.startInline){let c=1/0,u=e.slice(1),h;this.options.extensions.startInline.forEach(k=>{h=k.call({lexer:this},u),typeof h=="number"&&h>=0&&(c=Math.min(c,h))}),c<1/0&&c>=0&&(p=e.substring(0,c+1))}if(l=this.tokenizer.inlineText(p)){e=e.substring(l.raw.length),l.raw.slice(-1)!=="_"&&(o=l.raw.slice(-1)),i=!0;let c=n.at(-1);c?.type==="text"?(c.raw+=l.raw,c.text+=l.text):n.push(l);continue}if(e){let c="Infinite loop on byte: "+e.charCodeAt(0);if(this.options.silent){console.error(c);break}else throw new Error(c)}}return n}},Y=class{options;parser;constructor(r){this.options=r||P}space(r){return""}code({text:r,lang:e,escaped:n}){let s=(e||"").match(x.notSpaceStart)?.[0],t=r.replace(x.endingNewline,"")+`
`;return s?'<pre><code class="language-'+$(s)+'">'+(n?t:$(t,!0))+`</code></pre>
`:"<pre><code>"+(n?t:$(t,!0))+`</code></pre>
`}blockquote({tokens:r}){return`<blockquote>
${this.parser.parse(r)}</blockquote>
`}html({text:r}){return r}def(r){return""}heading({tokens:r,depth:e}){return`<h${e}>${this.parser.parseInline(r)}</h${e}>
`}hr(r){return`<hr>
`}list(r){let e=r.ordered,n=r.start,s="";for(let i=0;i<r.items.length;i++){let o=r.items[i];s+=this.listitem(o)}let t=e?"ol":"ul",a=e&&n!==1?' start="'+n+'"':"";return"<"+t+a+`>
`+s+"</"+t+`>
`}listitem(r){return`<li>${this.parser.parse(r.tokens)}</li>
`}checkbox({checked:r}){return"<input "+(r?'checked="" ':"")+'disabled="" type="checkbox"> '}paragraph({tokens:r}){return`<p>${this.parser.parseInline(r)}</p>
`}table(r){let e="",n="";for(let t=0;t<r.header.length;t++)n+=this.tablecell(r.header[t]);e+=this.tablerow({text:n});let s="";for(let t=0;t<r.rows.length;t++){let a=r.rows[t];n="";for(let i=0;i<a.length;i++)n+=this.tablecell(a[i]);s+=this.tablerow({text:n})}return s&&(s=`<tbody>${s}</tbody>`),`<table>
<thead>
`+e+`</thead>
`+s+`</table>
`}tablerow({text:r}){return`<tr>
${r}</tr>
`}tablecell(r){let e=this.parser.parseInline(r.tokens),n=r.header?"th":"td";return(r.align?`<${n} align="${r.align}">`:`<${n}>`)+e+`</${n}>
`}strong({tokens:r}){return`<strong>${this.parser.parseInline(r)}</strong>`}em({tokens:r}){return`<em>${this.parser.parseInline(r)}</em>`}codespan({text:r}){return`<code>${$(r,!0)}</code>`}br(r){return"<br>"}del({tokens:r}){return`<del>${this.parser.parseInline(r)}</del>`}link({href:r,title:e,tokens:n}){let s=this.parser.parseInline(n),t=Me(r);if(t===null)return s;r=t;let a='<a href="'+r+'"';return e&&(a+=' title="'+$(e)+'"'),a+=">"+s+"</a>",a}image({href:r,title:e,text:n,tokens:s}){s&&(n=this.parser.parseInline(s,this.parser.textRenderer));let t=Me(r);if(t===null)return $(n);r=t;let a=`<img src="${r}" alt="${$(n)}"`;return e&&(a+=` title="${$(e)}"`),a+=">",a}text(r){return"tokens"in r&&r.tokens?this.parser.parseInline(r.tokens):"escaped"in r&&r.escaped?r.text:$(r.text)}},ve=class{strong({text:r}){return r}em({text:r}){return r}codespan({text:r}){return r}del({text:r}){return r}html({text:r}){return r}text({text:r}){return r}link({text:r}){return""+r}image({text:r}){return""+r}br(){return""}checkbox({raw:r}){return r}},S=class ge{options;renderer;textRenderer;constructor(e){this.options=e||P,this.options.renderer=this.options.renderer||new Y,this.renderer=this.options.renderer,this.renderer.options=this.options,this.renderer.parser=this,this.textRenderer=new ve}static parse(e,n){return new ge(n).parse(e)}static parseInline(e,n){return new ge(n).parseInline(e)}parse(e){this.renderer.parser=this;let n="";for(let s=0;s<e.length;s++){let t=e[s];if(this.options.extensions?.renderers?.[t.type]){let i=t,o=this.options.extensions.renderers[i.type].call({parser:this},i);if(o!==!1||!["space","hr","heading","code","table","blockquote","list","html","def","paragraph","text"].includes(i.type)){n+=o||"";continue}}let a=t;switch(a.type){case"space":{n+=this.renderer.space(a);break}case"hr":{n+=this.renderer.hr(a);break}case"heading":{n+=this.renderer.heading(a);break}case"code":{n+=this.renderer.code(a);break}case"table":{n+=this.renderer.table(a);break}case"blockquote":{n+=this.renderer.blockquote(a);break}case"list":{n+=this.renderer.list(a);break}case"checkbox":{n+=this.renderer.checkbox(a);break}case"html":{n+=this.renderer.html(a);break}case"def":{n+=this.renderer.def(a);break}case"paragraph":{n+=this.renderer.paragraph(a);break}case"text":{n+=this.renderer.text(a);break}default:{let i='Token with "'+a.type+'" type was not found.';if(this.options.silent)return console.error(i),"";throw new Error(i)}}}return n}parseInline(e,n=this.renderer){this.renderer.parser=this;let s="";for(let t=0;t<e.length;t++){let a=e[t];if(this.options.extensions?.renderers?.[a.type]){let o=this.options.extensions.renderers[a.type].call({parser:this},a);if(o!==!1||!["escape","html","link","image","strong","em","codespan","br","del","text"].includes(a.type)){s+=o||"";continue}}let i=a;switch(i.type){case"escape":{s+=n.text(i);break}case"html":{s+=n.html(i);break}case"link":{s+=n.link(i);break}case"image":{s+=n.image(i);break}case"checkbox":{s+=n.checkbox(i);break}case"strong":{s+=n.strong(i);break}case"em":{s+=n.em(i);break}case"codespan":{s+=n.codespan(i);break}case"br":{s+=n.br(i);break}case"del":{s+=n.del(i);break}case"text":{s+=n.text(i);break}default:{let o='Token with "'+i.type+'" type was not found.';if(this.options.silent)return console.error(o),"";throw new Error(o)}}}return s}},N=class{options;block;constructor(r){this.options=r||P}static passThroughHooks=new Set(["preprocess","postprocess","processAllTokens","emStrongMask"]);static passThroughHooksRespectAsync=new Set(["preprocess","postprocess","processAllTokens"]);preprocess(r){return r}postprocess(r){return r}processAllTokens(r){return r}emStrongMask(r){return r}provideLexer(r=this.block){return r?y.lex:y.lexInline}provideParser(r=this.block){return r?S.parse:S.parseInline}},jn=class{defaults=de();options=this.setOptions;parse=this.parseMarkdown(!0);parseInline=this.parseMarkdown(!1);Parser=S;Renderer=Y;TextRenderer=ve;Lexer=y;Tokenizer=J;Hooks=N;constructor(...r){this.use(...r)}walkTokens(r,e){let n=[];for(let s of r)switch(n=n.concat(e.call(this,s)),s.type){case"table":{let t=s;for(let a of t.header)n=n.concat(this.walkTokens(a.tokens,e));for(let a of t.rows)for(let i of a)n=n.concat(this.walkTokens(i.tokens,e));break}case"list":{let t=s;n=n.concat(this.walkTokens(t.items,e));break}default:{let t=s;this.defaults.extensions?.childTokens?.[t.type]?this.defaults.extensions.childTokens[t.type].forEach(a=>{let i=t[a].flat(1/0);n=n.concat(this.walkTokens(i,e))}):t.tokens&&(n=n.concat(this.walkTokens(t.tokens,e)))}}return n}use(...r){let e=this.defaults.extensions||{renderers:{},childTokens:{}};return r.forEach(n=>{let s={...n};if(s.async=this.defaults.async||s.async||!1,n.extensions&&(n.extensions.forEach(t=>{if(!t.name)throw new Error("extension name required");if("renderer"in t){let a=e.renderers[t.name];a?e.renderers[t.name]=function(...i){let o=t.renderer.apply(this,i);return o===!1&&(o=a.apply(this,i)),o}:e.renderers[t.name]=t.renderer}if("tokenizer"in t){if(!t.level||t.level!=="block"&&t.level!=="inline")throw new Error("extension level must be 'block' or 'inline'");let a=e[t.level];a?a.unshift(t.tokenizer):e[t.level]=[t.tokenizer],t.start&&(t.level==="block"?e.startBlock?e.startBlock.push(t.start):e.startBlock=[t.start]:t.level==="inline"&&(e.startInline?e.startInline.push(t.start):e.startInline=[t.start]))}"childTokens"in t&&t.childTokens&&(e.childTokens[t.name]=t.childTokens)}),s.extensions=e),n.renderer){let t=this.defaults.renderer||new Y(this.defaults);for(let a in n.renderer){if(!(a in t))throw new Error(`renderer '${a}' does not exist`);if(["options","parser"].includes(a))continue;let i=a,o=n.renderer[i],l=t[i];t[i]=(...p)=>{let c=o.apply(t,p);return c===!1&&(c=l.apply(t,p)),c||""}}s.renderer=t}if(n.tokenizer){let t=this.defaults.tokenizer||new J(this.defaults);for(let a in n.tokenizer){if(!(a in t))throw new Error(`tokenizer '${a}' does not exist`);if(["options","rules","lexer"].includes(a))continue;let i=a,o=n.tokenizer[i],l=t[i];t[i]=(...p)=>{let c=o.apply(t,p);return c===!1&&(c=l.apply(t,p)),c}}s.tokenizer=t}if(n.hooks){let t=this.defaults.hooks||new N;for(let a in n.hooks){if(!(a in t))throw new Error(`hook '${a}' does not exist`);if(["options","block"].includes(a))continue;let i=a,o=n.hooks[i],l=t[i];N.passThroughHooks.has(a)?t[i]=p=>{if(this.defaults.async&&N.passThroughHooksRespectAsync.has(a))return(async()=>{let u=await o.call(t,p);return l.call(t,u)})();let c=o.call(t,p);return l.call(t,c)}:t[i]=(...p)=>{if(this.defaults.async)return(async()=>{let u=await o.apply(t,p);return u===!1&&(u=await l.apply(t,p)),u})();let c=o.apply(t,p);return c===!1&&(c=l.apply(t,p)),c}}s.hooks=t}if(n.walkTokens){let t=this.defaults.walkTokens,a=n.walkTokens;s.walkTokens=function(i){let o=[];return o.push(a.call(this,i)),t&&(o=o.concat(t.call(this,i))),o}}this.defaults={...this.defaults,...s}}),this}setOptions(r){return this.defaults={...this.defaults,...r},this}lexer(r,e){return y.lex(r,e??this.defaults)}parser(r,e){return S.parse(r,e??this.defaults)}parseMarkdown(r){return(e,n)=>{let s={...n},t={...this.defaults,...s},a=this.onError(!!t.silent,!!t.async);if(this.defaults.async===!0&&s.async===!1)return a(new Error("marked(): The async option was set to true by an extension. Remove async: false from the parse options object to return a Promise."));if(typeof e>"u"||e===null)return a(new Error("marked(): input parameter is undefined or null"));if(typeof e!="string")return a(new Error("marked(): input parameter is of type "+Object.prototype.toString.call(e)+", string expected"));if(t.hooks&&(t.hooks.options=t,t.hooks.block=r),t.async)return(async()=>{let i=t.hooks?await t.hooks.preprocess(e):e,o=await(t.hooks?await t.hooks.provideLexer(r):r?y.lex:y.lexInline)(i,t),l=t.hooks?await t.hooks.processAllTokens(o):o;t.walkTokens&&await Promise.all(this.walkTokens(l,t.walkTokens));let p=await(t.hooks?await t.hooks.provideParser(r):r?S.parse:S.parseInline)(l,t);return t.hooks?await t.hooks.postprocess(p):p})().catch(a);try{t.hooks&&(e=t.hooks.preprocess(e));let i=(t.hooks?t.hooks.provideLexer(r):r?y.lex:y.lexInline)(e,t);t.hooks&&(i=t.hooks.processAllTokens(i)),t.walkTokens&&this.walkTokens(i,t.walkTokens);let o=(t.hooks?t.hooks.provideParser(r):r?S.parse:S.parseInline)(i,t);return t.hooks&&(o=t.hooks.postprocess(o)),o}catch(i){return a(i)}}}onError(r,e){return n=>{if(n.message+=`
Please report this to https://github.com/markedjs/marked.`,r){let s="<p>An error occurred:</p><pre>"+$(n.message+"",!0)+"</pre>";return e?Promise.resolve(s):s}if(e)return Promise.reject(n);throw n}}},C=new jn;function f(r,e){return C.parse(r,e)}f.options=f.setOptions=function(r){return C.setOptions(r),f.defaults=C.defaults,Oe(f.defaults),f};f.getDefaults=de;f.defaults=P;f.use=function(...r){return C.use(...r),f.defaults=C.defaults,Oe(f.defaults),f};f.walkTokens=function(r,e){return C.walkTokens(r,e)};f.parseInline=C.parseInline;f.Parser=S;f.parser=S.parse;f.Renderer=Y;f.TextRenderer=ve;f.Lexer=y;f.lexer=y.lex;f.Tokenizer=J;f.Hooks=N;f.parse=f;f.options;f.setOptions;f.use;f.walkTokens;f.parseInline;S.parse;y.lex;var Dn=Z('<span class="rounded px-1.5 py-0.5 text-[10px] font-semibold tracking-wider uppercase" style="background:#1a1a1a; color:#f97316;">Rust</span>'),Nn=Z('<details class="group overflow-hidden rounded-xl" style="background:#0d0d0d; border:1px solid #1a1a1a;"><summary class="flex cursor-pointer items-center justify-between px-5 py-3.5 transition-colors select-none hover:bg-neutral-800/50"><div class="flex items-center gap-3"><span class="font-mono text-sm font-medium text-red-400"> </span> <!></div> <svg class="h-4 w-4 text-neutral-500 transition-transform duration-200 group-open:rotate-180" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path></svg></summary> <div class="border-t" style="border-color:#1a1a1a;"><pre class="!m-0 !rounded-none !border-0"><code> </code></pre></div></details>'),Zn=Z('<div class="mt-12 border-t pt-8" style="border-color:#1a1a1a;"><h3 class="mb-2 flex items-center gap-2 text-xl font-bold text-white"><span class="text-red-500">📄</span> Chapter Scripts</h3> <p class="mb-6 text-sm text-neutral-500">Full source code for this chapter. Run with <code>kiro filename.kiro</code></p> <div class="space-y-4"></div></div>'),Hn=Z("<!> <!>",1),Fn=Z('<div class="rounded-lg border border-red-900/50 bg-red-900/20 p-4 text-red-400">Chapter not found. <code> </code></div>');function sr(r,e){yt(e,!1);const n=()=>Ct(Bt,"$page",s),[s,t]=zt(),a=W(),i=W(),o=W(),l=W(),p=Object.assign({"/src/lib/content/tutorial/chapter-00/00_installation.md":ht,"/src/lib/content/tutorial/chapter-01/01_basics.md":ut,"/src/lib/content/tutorial/chapter-02/02_control_flow.md":pt,"/src/lib/content/tutorial/chapter-03/03_functions.md":ct,"/src/lib/content/tutorial/chapter-04/04_data.md":ot,"/src/lib/content/tutorial/chapter-05/05_errors.md":lt,"/src/lib/content/tutorial/chapter-06/06_advanced.md":at,"/src/lib/content/tutorial/chapter-06/06_async.md":it,"/src/lib/content/tutorial/chapter-07/07_pipes.md":st,"/src/lib/content/tutorial/chapter-08/08_pointers.md":rt,"/src/lib/content/tutorial/chapter-09/09_host_rust.md":nt,"/src/lib/content/tutorial/chapter-10/10_host_kiro.md":tt,"/src/lib/content/tutorial/final-project/final_project.md":et}),c=Object.assign({"/src/lib/content/tutorial/chapter-01/01_basics.kiro":jt,"/src/lib/content/tutorial/chapter-02/02_control_flow.kiro":Dt,"/src/lib/content/tutorial/chapter-03/03_functions.kiro":Nt,"/src/lib/content/tutorial/chapter-03/mylib.kiro":Zt,"/src/lib/content/tutorial/chapter-04/04_data.kiro":Ht,"/src/lib/content/tutorial/chapter-05/05_errors.kiro":Ft,"/src/lib/content/tutorial/chapter-06/06_advanced.kiro":Qt,"/src/lib/content/tutorial/chapter-06/06_async.kiro":Gt,"/src/lib/content/tutorial/chapter-07/07_pipes.kiro":Vt,"/src/lib/content/tutorial/chapter-08/08_pointers.kiro":Wt,"/src/lib/content/tutorial/chapter-10/10_host.kiro":Ut,"/src/lib/content/tutorial/final-project/final_project.kiro":Kt}),u=Object.assign({"/src/lib/content/tutorial/chapter-10/10_host.rs":Xt});function h(m){return m.replace(/\]\(\.\.\/([^/]+)\/[^)]+\.md\)/g,`](${Mt}/tutorial/$1)`)}G(()=>n(),()=>{V(a,n().params.slug)}),G(()=>b(a),()=>{V(i,Object.entries(p).find(([m])=>m.includes(`/${b(a)}/`))?.[1]||"")}),G(()=>b(i),()=>{V(o,f(h(b(i))))}),G(()=>b(a),()=>{V(l,[...Object.entries(c).filter(([m])=>m.includes(`/${b(a)}/`)).map(([m,_])=>({name:m.split("/").pop()||"",content:_,lang:"kiro"})),...Object.entries(u).filter(([m])=>m.includes(`/${b(a)}/`)).map(([m,_])=>({name:m.split("/").pop()||"",content:_,lang:"rust"}))])}),St(),Lt();var k=gt();Et("1sirout",m=>{$t(()=>{Tt.title=`Kiro Tutorial - ${b(a)??""}`})});var d=Pe(k);{var w=m=>{var _=Hn(),A=Pe(_);qt(A,()=>b(o));var M=O(A,2);{var B=T=>{var ne=Zn(),ye=O(R(ne),4);Pt(ye,5,()=>b(l),It,(We,I)=>{var re=Nn(),se=R(re),Se=R(se),ie=R(Se),Ue=R(ie,!0);v(ie);var Ke=O(ie,2);{var Xe=ae=>{var Ye=Dn();E(ae,Ye)};ce(Ke,ae=>{b(I),U(()=>b(I).lang==="rust")&&ae(Xe)})}v(Se),At(2),v(se);var Re=O(se,2),$e=R(Re),Te=R($e),Je=R(Te,!0);v(Te),v($e),v(Re),v(re),pe(()=>{oe(Ue,(b(I),U(()=>b(I).name))),oe(Je,(b(I),U(()=>b(I).content)))}),E(We,re)}),v(ye),v(ne),E(T,ne)};ce(M,T=>{b(l),U(()=>b(l).length>0)&&T(B)})}E(m,_)},F=m=>{var _=Fn(),A=O(R(_)),M=R(A,!0);v(A),v(_),pe(()=>oe(M,b(a))),E(m,_)};ce(d,m=>{b(i)?m(w):m(F,!1)})}E(r,k),Rt(),t()}export{sr as component,rr as universal};
