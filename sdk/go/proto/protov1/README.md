**TODO**

- periodic ping messages with stats (app level ping)
- periodic statistics
- hangup action

**application.move**

- -> req: application.move
- (...) app is moved
- <- req: session.terminate
- -> res: session.terminate
- <- res: application.move
- <- evt: session.terminated

Log all events to file for certain use cases
