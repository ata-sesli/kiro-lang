<script>
	import { base } from '$app/paths';
</script>

<svelte:head>
	<title>Kiro — Modern Programming Language</title>
	<meta
		name="description"
		content="Kiro is a modern, experimental programming language with a dual-execution model: Interpreter + Rust Transpiler. Safe by default, async-first, with zero-friction Rust FFI."
	/>
</svelte:head>

<!-- ════════════════════════════════════════════════
     HERO
     ════════════════════════════════════════════════ -->
<section class="relative overflow-hidden">
	<!-- Background glow -->
	<div
		class="absolute top-1/2 left-1/2 h-[600px] w-[600px] -translate-x-1/2 -translate-y-1/2 rounded-full opacity-15 blur-[120px]"
		style="background:radial-gradient(circle, #dc2626 0%, transparent 70%);"
	></div>

	<div class="relative mx-auto max-w-5xl px-4 pt-24 pb-20 text-center sm:px-6">
		<div class="mb-8 flex justify-center">
			<img
				src="{base}/kiro-logo.png"
				alt="Kiro Logo"
				class="h-32 w-32 drop-shadow-2xl md:h-40 md:w-40"
			/>
		</div>
		<h1 class="text-5xl leading-[1.1] font-black tracking-tight text-white sm:text-6xl md:text-7xl">
			The <span class="text-red-500">Kiro</span> Language
		</h1>
		<p class="mx-auto mt-6 max-w-2xl text-lg leading-relaxed text-neutral-400 md:text-xl">
			A modern, experimental programming language exploring the intersection of
			<span class="font-medium text-neutral-200">high-level expressivity</span> and
			<span class="font-medium text-neutral-200">low-level performance</span>.
			<br class="hidden sm:block" />
			Dual-execution: Interpreter + Transpiler to Rust.
		</p>
		<div class="mt-10 flex flex-wrap justify-center gap-3">
			<a
				href="{base}/tutorial"
				class="inline-flex items-center gap-2 rounded-lg bg-red-600 px-6 py-2.5 text-sm font-semibold text-white shadow-lg transition-all hover:bg-red-500 hover:shadow-red-600/25"
			>
				Get Started
				<svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"
					><path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="2"
						d="M13 7l5 5m0 0l-5 5m5-5H6"
					/></svg
				>
			</a>
			<a
				href="https://github.com/ata-sesli/kiro-lang"
				target="_blank"
				class="inline-flex items-center gap-2 rounded-lg border px-6 py-2.5 text-sm font-semibold text-neutral-300 transition-all hover:border-neutral-500 hover:text-white"
				style="border-color:#262626; background:#111111;"
			>
				<svg class="h-4 w-4" fill="currentColor" viewBox="0 0 24 24"
					><path
						fill-rule="evenodd"
						d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z"
						clip-rule="evenodd"
					/></svg
				>
				View on GitHub
			</a>
		</div>
	</div>
</section>

<!-- ════════════════════════════════════════════════
     KEY FEATURES
     ════════════════════════════════════════════════ -->
<section class="border-t" style="border-color:#1a1a1a;">
	<div class="mx-auto max-w-6xl px-4 py-20 sm:px-6">
		<h2 class="mb-4 text-center text-3xl font-bold text-white">Key Features</h2>
		<p class="mx-auto mb-14 max-w-xl text-center text-neutral-500">
			Everything you need from a modern language, nothing you don't.
		</p>

		<div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
			{#each [{ icon: '🔒', title: 'Safe by Default', desc: 'Variables are immutable unless explicitly declared with var.' }, { icon: '✨', title: 'Pure Functions', desc: 'Strict pure fn keyword ensures side-effect free, deterministic code.' }, { icon: '📦', title: 'Module System', desc: 'Organize code across files with import and qualified access.' }, { icon: '🔄', title: 'Expressive Loops', desc: 'Powerful loop constructs with built-in filtering (on) and stepping (per).' }, { icon: '🎯', title: 'Pointers Made Easy', desc: 'Simple ref/deref syntax compiling to safe Rust primitives (Arc<Mutex>).' }, { icon: '⚡', title: 'Async First', desc: 'Built-in run keyword to spawn asynchronous tasks effortlessly.' }, { icon: '🦀', title: 'Rust FFI', desc: 'Type-safe access to the Rust ecosystem via rust fn and shared runtime.' }, { icon: '🚀', title: 'Transpiled to Rust', desc: 'Compiled to idiomatic Rust — full ecosystem and native performance.' }] as feature}
				<div
					class="rounded-xl p-5 transition-all duration-200 hover:scale-[1.02]"
					style="background:#111111; border:1px solid #1a1a1a;"
				>
					<div class="mb-3 text-2xl">{feature.icon}</div>
					<h3 class="mb-1.5 text-sm font-semibold text-white">{feature.title}</h3>
					<p class="text-xs leading-relaxed text-neutral-500">{feature.desc}</p>
				</div>
			{/each}
		</div>
	</div>
</section>

<!-- ════════════════════════════════════════════════
     QUICK LOOK — CODE EXAMPLES
     ════════════════════════════════════════════════ -->
<section class="border-t" style="border-color:#1a1a1a;">
	<div class="mx-auto max-w-6xl px-4 py-20 sm:px-6">
		<h2 class="mb-4 text-center text-3xl font-bold text-white">A Quick Look</h2>
		<p class="mx-auto mb-14 max-w-xl text-center text-neutral-500">
			Clean, readable syntax designed for humans first.
		</p>

		<div class="grid grid-cols-1 gap-6 lg:grid-cols-2">
			<!-- Variables & Types -->
			<div class="overflow-hidden rounded-xl" style="background:#111111; border:1px solid #1a1a1a;">
				<div class="flex items-center gap-2 border-b px-5 py-3" style="border-color:#1a1a1a;">
					<span class="text-xs font-semibold tracking-wider text-red-500 uppercase"
						>Variables & Types</span
					>
				</div>
				<pre class="!m-0 !rounded-none !border-0"><code
						>pi = 3.14        // Immutable
name = "Kiro"    // Immutable

var count = 0
count = count + 1 // OK — mutable with var</code
					></pre>
				<div class="px-5 py-3 text-xs text-neutral-500">
					Supported: <code>num</code>, <code>str</code>, <code>bool</code>, <code>void</code>,
					<code>adr &lt;type&gt;</code>, <code>pipe &lt;type&gt;</code>,
					<code>list &lt;type&gt;</code>, <code>map &lt;key&gt; &lt;val&gt;</code>, Structs
				</div>
			</div>

			<!-- Functions -->
			<div class="overflow-hidden rounded-xl" style="background:#111111; border:1px solid #1a1a1a;">
				<div class="flex items-center gap-2 border-b px-5 py-3" style="border-color:#1a1a1a;">
					<span class="text-xs font-semibold tracking-wider text-red-500 uppercase"
						>Functions & Purity</span
					>
				</div>
				<pre class="!m-0 !rounded-none !border-0"><code
						>fn add(a: num, b: num) -> num {'{'} 
    return a + b
{'}'}

pure fn square(x: num) -> num {'{'}
    return x * x   // No side-effects allowed
{'}'}</code
					></pre>
			</div>

			<!-- Structs -->
			<div class="overflow-hidden rounded-xl" style="background:#111111; border:1px solid #1a1a1a;">
				<div class="flex items-center gap-2 border-b px-5 py-3" style="border-color:#1a1a1a;">
					<span class="text-xs font-semibold tracking-wider text-red-500 uppercase"
						>Structs & Mutation</span
					>
				</div>
				<pre class="!m-0 !rounded-none !border-0"><code
						>struct User {'{'}
    name: str
    age: num
{'}'}

var player = User {'{'} name: "Hero", age: 20 {'}'}
player.age = 21  // Field mutation with var</code
					></pre>
			</div>

			<!-- Control Flow -->
			<div class="overflow-hidden rounded-xl" style="background:#111111; border:1px solid #1a1a1a;">
				<div class="flex items-center gap-2 border-b px-5 py-3" style="border-color:#1a1a1a;">
					<span class="text-xs font-semibold tracking-wider text-red-500 uppercase"
						>Control Flow</span
					>
				</div>
				<pre class="!m-0 !rounded-none !border-0"><code
						>on (x > 10) {'{'}
    print "High"
{'}'} off {'{'}
    print "Low"
{'}'}

loop i in 0..10 {'{'} print i {'}'}
loop x in list per 2 on (x > 5) {'{'} ... {'}'}</code
					></pre>
			</div>

			<!-- Concurrency -->
			<div class="overflow-hidden rounded-xl" style="background:#111111; border:1px solid #1a1a1a;">
				<div class="flex items-center gap-2 border-b px-5 py-3" style="border-color:#1a1a1a;">
					<span class="text-xs font-semibold tracking-wider text-red-500 uppercase"
						>Concurrency & Pipes</span
					>
				</div>
				<pre class="!m-0 !rounded-none !border-0"><code
						>run worker(id)        // Spawn async task

var p = pipe num      // Typed channel
give p 42             // Send
var x = take p        // Receive (awaits)
close p               // Done</code
					></pre>
			</div>

			<!-- Error Handling -->
			<div class="overflow-hidden rounded-xl" style="background:#111111; border:1px solid #1a1a1a;">
				<div class="flex items-center gap-2 border-b px-5 py-3" style="border-color:#1a1a1a;">
					<span class="text-xs font-semibold tracking-wider text-red-500 uppercase"
						>Error Handling</span
					>
				</div>
				<pre class="!m-0 !rounded-none !border-0"><code
						>error NotFound = "File not found"

fn load(code: num) -> str! {'{'}
    on (code == 1) {'{'} return NotFound {'}'}
    return "OK"
{'}'}

var r = load(1)
on (r) {'{'} print r {'}'}
error NotFound {'{'} print "Not found" {'}'}</code
					></pre>
			</div>
		</div>
	</div>
</section>

<!-- ════════════════════════════════════════════════
     MODULES & HOST (Rust FFI)
     ════════════════════════════════════════════════ -->
<section class="border-t" style="border-color:#1a1a1a;">
	<div class="mx-auto max-w-6xl px-4 py-20 sm:px-6">
		<div class="grid grid-cols-1 items-start gap-12 lg:grid-cols-2">
			<div>
				<h2 class="mb-4 text-3xl font-bold text-white">Module System</h2>
				<p class="mb-6 leading-relaxed text-neutral-400">
					Any <code>.kiro</code> file in the same directory can be imported. Access exported members
					with qualified syntax. Kiro also ships with an
					<span class="font-medium text-white">embedded standard library</span>
					(<code>std_fs</code>, <code>std_net</code>, <code>std_env</code>, <code>std_io</code>,
					<code>std_time</code>) baked directly into the binary for zero-configuration portability.
				</p>
				<pre><code
						>// math.kiro
fn add(a: num, b: num) {'{'}
    return a + b
{'}'}

// main.kiro
import math

fn main() {'{'}
    var result = math.add(10, 20)
    print result
{'}'}
main()</code
					></pre>
			</div>
			<div>
				<h2 class="mb-4 text-3xl font-bold text-white">
					Host Modules <span class="text-red-500">(Rust FFI)</span>
				</h2>
				<p class="mb-6 leading-relaxed text-neutral-400">
					Zero-friction access to the Rust ecosystem. Declare external functions with the <code
						>rust</code
					>
					keyword. The logic lives in a centralized
					<code>header.rs</code> glue file using the <code>kiro_runtime</code> crate.
				</p>
				<pre><code
						>// Kiro side
rust fn read_file(path: str) -> str!

// header.rs (Rust glue)
use kiro_runtime::{'{'}RuntimeVal, KiroError{'}'};

pub fn read_file(args: Vec&lt;RuntimeVal&gt;)
    -> Result&lt;RuntimeVal, KiroError&gt; {'{'}
    let path = args[0].as_str()?;
    match std::fs::read_to_string(path) {'{'}
        Ok(c) => Ok(RuntimeVal::from(c)),
        Err(_) => Err(KiroError::new("NotFound")),
    {'}'}
{'}'}</code
					></pre>
			</div>
		</div>
	</div>
</section>

<!-- ════════════════════════════════════════════════
     ARCHITECTURE
     ════════════════════════════════════════════════ -->
<section class="border-t" style="border-color:#1a1a1a;">
	<div class="mx-auto max-w-5xl px-4 py-20 sm:px-6">
		<h2 class="mb-4 text-center text-3xl font-bold text-white">Architecture</h2>
		<p class="mx-auto mb-14 max-w-xl text-center text-neutral-500">
			Kiro uses a <span class="font-medium text-white">Double Pass</span> system for maximum safety and
			performance.
		</p>

		<div class="grid grid-cols-1 gap-6">
			<div class="rounded-xl p-8" style="background:#111111; border:1px solid #1a1a1a;">
				<div class="mb-4 flex items-center gap-3">
					<div
						class="flex h-10 w-10 items-center justify-center rounded-lg text-lg font-bold text-white"
						style="background:#dc2626;"
					>
						1
					</div>
					<h3 class="text-lg font-semibold text-white">Interpreter</h3>
				</div>
				<ul class="space-y-3 text-base text-neutral-400">
					<li class="flex gap-2">
						<span class="mt-0.5 text-red-500">›</span> Walks the AST and maintains a runtime environment
					</li>
					<li class="flex gap-2">
						<span class="mt-0.5 text-red-500">›</span> <strong class="text-white">Simulator</strong>
						provides mock responses for <code>rust fn</code> — validates without Rust compilation
					</li>
					<li class="flex gap-2">
						<span class="mt-0.5 text-red-500">›</span> Recursively loads and executes imported modules
						in isolation
					</li>
				</ul>
			</div>
			<div class="rounded-xl p-8" style="background:#111111; border:1px solid #1a1a1a;">
				<div class="mb-4 flex items-center gap-3">
					<div
						class="flex h-10 w-10 items-center justify-center rounded-lg text-lg font-bold text-white"
						style="background:#dc2626;"
					>
						2
					</div>
					<h3 class="text-lg font-semibold text-white">Transpiler</h3>
				</div>
				<ul class="space-y-3 text-base text-neutral-400">
					<li class="flex gap-2">
						<span class="mt-0.5 text-red-500">›</span> Converts Kiro to idiomatic
						<strong class="text-white">Rust</strong>
					</li>
					<li class="flex gap-2">
						<span class="mt-0.5 text-red-500">›</span>
						<strong class="text-white">Recursive Build</strong>
						— identifies dependencies and compiles as Rust modules (<code>pub mod</code>)
					</li>
					<li class="flex gap-2">
						<span class="mt-0.5 text-red-500">›</span> Hoists struct definitions and imports for valid
						Rust output
					</li>
				</ul>
			</div>
		</div>
	</div>
</section>

<!-- ════════════════════════════════════════════════
     RUNTIME (kiro_runtime crate)
     ════════════════════════════════════════════════ -->
<section class="border-t" style="border-color:#1a1a1a;">
	<div class="mx-auto max-w-5xl px-4 py-20 sm:px-6">
		<h2 class="mb-4 text-center text-3xl font-bold text-white">The Runtime</h2>
		<p class="mx-auto mb-14 max-w-xl text-center text-neutral-500">
			<code>kiro_runtime</code> — a small Rust crate defining shared data structures at the Kiro ↔ Rust
			boundary.
		</p>

		<div class="mb-10 grid grid-cols-1 gap-6">
			<div class="rounded-xl p-8" style="background:#111111; border:1px solid #1a1a1a;">
				<h3 class="mb-4 text-lg font-semibold text-white">Runtime Value Model</h3>
				<p class="mb-4 text-base text-neutral-400">
					<code>RuntimeVal</code> represents Kiro values in Rust:
				</p>
				<div class="space-y-2 font-mono text-base text-neutral-400">
					<div><span class="text-red-400">Num</span>(f64)</div>
					<div><span class="text-red-400">Str</span>(String)</div>
					<div><span class="text-red-400">Bool</span>(bool)</div>
					<div><span class="text-red-400">List</span>(Vec&lt;RuntimeVal&gt;)</div>
					<div><span class="text-red-400">Map</span>(HashMap&lt;String, RuntimeVal&gt;)</div>
					<div><span class="text-red-400">Void</span></div>
				</div>
			</div>
			<div class="rounded-xl p-8" style="background:#111111; border:1px solid #1a1a1a;">
				<h3 class="mb-4 text-lg font-semibold text-white">Conversions</h3>
				<p class="mb-4 text-base text-neutral-400">Bi-directional type conversion:</p>
				<div class="space-y-3 text-base text-neutral-400">
					<div>
						<span class="font-medium text-neutral-200">Rust → RuntimeVal:</span>
						<span class="font-mono"> f64, String, &str, bool, (), Vec&lt;T&gt;</span>
					</div>
					<div>
						<span class="font-medium text-neutral-200">RuntimeVal → Rust:</span>
						<span class="font-mono"> TryFrom for String, f64, bool, ()</span>
					</div>
					<div class="mt-2 font-medium text-neutral-200">Accessor helpers:</div>
					<div class="font-mono">
						<code>.as_str()</code> &nbsp; <code>.as_num()</code> &nbsp; <code>.as_bool()</code>
					</div>
				</div>
			</div>
			<div class="rounded-xl p-8" style="background:#111111; border:1px solid #1a1a1a;">
				<h3 class="mb-4 text-lg font-semibold text-white">Error Type</h3>
				<p class="mb-4 text-base text-neutral-400">
					<code>KiroError</code> — minimal error type for glue-level failures.
				</p>
				<pre><code
						>KiroError::new("TypeError")
// Display + std::error::Error</code
					></pre>
				<p class="mt-4 text-base text-neutral-500">
					Keeps host boundary contracts explicit, type-checked, and stable.
				</p>
			</div>
		</div>
	</div>
</section>

<!-- ════════════════════════════════════════════════
     INSTALLATION
     ════════════════════════════════════════════════ -->
<section class="border-t" style="border-color:#1a1a1a;">
	<div class="mx-auto max-w-5xl px-4 py-20 sm:px-6">
		<h2 class="mb-4 text-center text-3xl font-bold text-white">Get Started</h2>
		<p class="mb-12 text-center text-lg text-neutral-500">Value your time? Just clone and run.</p>

		<div class="grid grid-cols-1 gap-6">
			<div class="rounded-xl p-8" style="background:#111111; border:1px solid #1a1a1a;">
				<h3 class="mb-2 text-lg font-semibold text-white">Prerequisites</h3>
				<p class="mb-6 text-base text-neutral-400">
					<a
						href="https://rustup.rs/"
						target="_blank"
						class="text-red-400 underline hover:text-red-300">Rust & Cargo</a
					> installed.
				</p>

				<h3 class="mb-3 text-lg font-semibold text-white">Install</h3>
				<pre><code
						>cargo build --release
cp target/release/kiro-lang /usr/local/bin/kiro</code
					></pre>
			</div>
			<div class="rounded-xl p-8" style="background:#111111; border:1px solid #1a1a1a;">
				<h3 class="mb-4 text-lg font-semibold text-white">Usage</h3>
				<pre><code
						># Full Pipeline: Interpret → Compile → Execute
kiro main.kiro

# Fast Validation: Interpret ONLY
kiro check main.kiro

# Production Build: Compile ONLY
kiro build main.kiro

# Skip interpreter validation
kiro main.kiro --no-interpret

# Show compiler logs
kiro main.kiro --verbose</code
					></pre>
			</div>
		</div>
	</div>
</section>
