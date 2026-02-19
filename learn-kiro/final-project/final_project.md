# Final Project: Async Task Manager

This project is the point where core Kiro concepts stop being isolated exercises and become a coherent application. You will model task data with structs, process work concurrently with `run`, move information through pipes, and handle failures explicitly.

A practical architecture begins with a `Task` struct and a worker function that receives tasks and sends results. Multiple workers can run concurrently while a coordinating loop in main collects outputs and prints status.

A recommended progression is to build the system in stages. First, run one worker with one task and verify end-to-end flow. Then introduce multiple tasks. Next, scale to multiple workers. Finally, add explicit failure handling and aggregate reporting.

Throughout implementation, keep channel ownership clear: producers close the channels they own, consumers terminate predictably, and every `take` has a known source path.

Run the project with:

```bash
kiro final_project.kiro
```

Once the baseline works, extend it with retries, priorities, or separate success/error output channels.

## Common Pitfalls

A common project-level issue is implementing concurrency and error handling simultaneously before baseline flow works. The correct method is to lock in a minimal single-worker path first, then add complexity incrementally.

Another frequent problem is unclear pipe lifecycle ownership. The correct method is to assign ownership explicitly so each pipe is closed exactly once by the correct producer.

Teams also often under-test non-happy paths. The correct method is to force at least one expected failure path per stage and verify that the system still drains and exits cleanly.
