# Chapter 0: Installing Kiro

Before writing your first Kiro program, set up a stable local environment so every chapter in this guide runs exactly as described. Kiro is distributed as a command-line binary, so the installation goal is simple: you should be able to run `kiro --version` in your terminal from any project directory.

Start by obtaining the Kiro binary for your operating system from your project's release process. Place it in a directory that is part of your shell `PATH`, then open a new terminal session and verify the installation:

```bash
kiro --version
```

If the command prints a version string, your installation is complete. If the shell says the command is not found, the binary is either not executable or not in your `PATH`.

On Unix-like systems (macOS/Linux), make sure the file has execute permissions:

```bash
chmod +x /path/to/kiro
```

Then add its folder to `PATH` in your shell profile (`~/.zshrc`, `~/.bashrc`, etc.), reload the shell, and test again.

Now create a small working directory for this book:

```bash
mkdir kiro-playground
cd kiro-playground
```

Create a first program file named `hello.kiro`:

```kiro
print "Hello, Kiro!"
```

Run it:

```bash
kiro hello.kiro
```

You should see the greeting printed to your terminal. This confirms that editing, running, and reading output all work correctly.

## Common Pitfalls

A frequent issue is installing multiple binaries and accidentally using an older one. The correct approach is to run `which kiro` (or your shell equivalent) and verify it points to the intended binary location.

Another issue is editing shell configuration but forgetting to restart or reload the shell. The correct method is to run `source ~/.zshrc` (or reopen terminal) before re-checking `kiro --version`.

Developers also often run examples from the wrong directory and then assume the language is broken when files are not found. The correct habit is to confirm your current directory with `pwd` and keep chapter files together in a dedicated workspace.

## Next Step

Continue with [Chapter 1: The Basics](../chapter-01/01_basics.md).
