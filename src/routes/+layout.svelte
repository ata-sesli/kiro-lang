<script>
	import '../app.css';
	import { base } from '$app/paths';
	let isMenuOpen = false;
</script>

<svelte:head>
	<link rel="preconnect" href="https://fonts.googleapis.com" />
	<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
	<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800;900&family=JetBrains+Mono:wght@400;500;700&display=swap" rel="stylesheet" />
</svelte:head>

<div class="min-h-screen flex flex-col" style="background:#0a0a0a; color:#e5e5e5;">

	<!-- ─── Navbar ─── -->
	<nav class="fixed top-0 z-50 w-full border-b" style="border-color:#1a1a1a; background:rgba(10,10,10,0.85); backdrop-filter:blur(16px);">
		<div class="mx-auto max-w-6xl px-4 sm:px-6 lg:px-8">
			<div class="flex h-14 items-center justify-between">
				<!-- Logo -->
				<a href="{base}/" class="flex items-center gap-2.5 group">
					<img src="{base}/kiro-logo.png" alt="Kiro" class="h-7 w-7" />
					<span class="text-lg font-bold tracking-tight text-white group-hover:text-red-500 transition-colors duration-200">KIRO</span>
				</a>
				<!-- Desktop links -->
				<div class="hidden md:flex items-center gap-1">
					<a href="{base}/" class="px-3 py-1.5 rounded-md text-sm font-medium text-neutral-400 hover:text-white transition-colors">Home</a>
					<a href="{base}/tutorial" class="px-3 py-1.5 rounded-md text-sm font-medium text-neutral-400 hover:text-white transition-colors">Tutorial</a>
					<a href="{base}/extension" class="px-3 py-1.5 rounded-md text-sm font-medium text-neutral-400 hover:text-white transition-colors">VS Code Extension</a>
					<a href="https://github.com/ata-sesli/kiro-lang" target="_blank" rel="noopener noreferrer"
					   class="ml-2 flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium text-neutral-400 hover:text-white transition-colors">
						<svg class="h-4 w-4" fill="currentColor" viewBox="0 0 24 24"><path fill-rule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clip-rule="evenodd"/></svg>
						GitHub
					</a>
				</div>
				<!-- Mobile toggle -->
				<button on:click={() => isMenuOpen = !isMenuOpen} class="md:hidden p-2 rounded-md text-neutral-400 hover:text-white hover:bg-neutral-800 transition-colors" aria-label="Toggle menu">
					{#if !isMenuOpen}
						<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16"/></svg>
					{:else}
						<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
					{/if}
				</button>
			</div>
		</div>
		<!-- Mobile menu -->
		{#if isMenuOpen}
			<div class="md:hidden border-t" style="border-color:#1a1a1a; background:#0a0a0a;">
				<div class="px-3 py-3 space-y-1">
					<a href="{base}/" on:click={() => isMenuOpen = false} class="block px-3 py-2 rounded-md text-sm font-medium text-neutral-300 hover:text-white hover:bg-neutral-800">Home</a>
					<a href="{base}/tutorial" on:click={() => isMenuOpen = false} class="block px-3 py-2 rounded-md text-sm font-medium text-neutral-300 hover:text-white hover:bg-neutral-800">Tutorial</a>
					<a href="{base}/extension" on:click={() => isMenuOpen = false} class="block px-3 py-2 rounded-md text-sm font-medium text-neutral-300 hover:text-white hover:bg-neutral-800">VS Code Extension</a>
					<a href="https://github.com/ata-sesli/kiro-lang" target="_blank" class="block px-3 py-2 rounded-md text-sm font-medium text-neutral-300 hover:text-white hover:bg-neutral-800">GitHub</a>
				</div>
			</div>
		{/if}
	</nav>

	<!-- ─── Content ─── -->
	<main class="flex-1 pt-14">
		<slot />
	</main>

	<!-- ─── Footer ─── -->
	<footer class="border-t mt-auto" style="border-color:#1a1a1a;">
		<div class="max-w-6xl mx-auto px-4 py-8 flex flex-col sm:flex-row items-center justify-between gap-4">
			<p class="text-sm text-neutral-600">
				Built with <span class="text-red-500">❤️</span> in Rust &nbsp;·&nbsp; &copy; {new Date().getFullYear()} Kiro Language
			</p>
			<a href="https://github.com/ata-sesli/kiro-lang" target="_blank" rel="noopener noreferrer" aria-label="GitHub" class="text-neutral-600 hover:text-white transition-colors">
				<svg class="h-5 w-5" fill="currentColor" viewBox="0 0 24 24"><path fill-rule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clip-rule="evenodd"/></svg>
			</a>
		</div>
	</footer>
</div>
