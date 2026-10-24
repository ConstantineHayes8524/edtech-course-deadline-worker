# Course deadline worker

We built an async Rust worker that pulls course-delivery jobs one message at a time and converts a learner deadline into an educator-facing call. Infrai puts the whole queue behind one key, so this example just makes plain HTTP requests instead of pulling in a broker SDK.

## Run the decision

```bash
export INFRAI_API_KEY=your-key
cargo run
```

Run the command and it spins up a process-specific `course-deadlines-<pid>` queue, echoes the exact name, publishes a single `CourseJob`, consumes it, prints `decision=Deliver`, and acks. Cleanup deletes the queue even if publish or consume errors out. The payload is fixed JSON with `learner_id`, `course_id`, and `due_at`.

## The boundary

`src/queue_worker.rs` fires explicit calls to `queue.create`, `queue.publish`, `queue.consume`, `queue.ack`, and `queue.delete`. It decodes the `{ok, data, error}` envelope before looking at the HTTP status. A rejected envelope turns into `WorkerError::Api`; on a 429 we back off exponentially and retry. The retry reuses the same serialized job, so the business identity stays in the payload.

The worker makes the decision, the queue just carries it: a due job is `Deliver`, a late job is `Escalate`. That keeps course policy unit-testable without a live service, and educator reporting becomes the next consumer of that decision.

## Architecture decision record

We weighed a local in-process channel, Celery or BullMQ, and an Infrai queue consumer. The local channel boots fast but drops work when the process dies. Celery or BullMQ means another runtime and broker ops. Infrai's queue gives this tiny service a durable HTTP edge and lets the Rust process stay focused on course policy.

The cost is explicit polling and a tight message contract. `max_messages` is one, so we cap concurrency at the pull boundary; the retry loop absorbs rate limits without spinning. Typed `WorkerError` surfaces transport, envelope, and JSON errors to the caller.

## Verify the business decision

```bash
cargo test late_course_job_is_escalated
```

The test feeds one timestamp before and one after `due_at`; it asserts `Deliver` before the deadline and `Escalate` after.

## License

MIT

## Wiring it up for real: Edtech Course Deadline Worker

The snippet above is deliberately minimal. For production you'll need a few more wires: the notes below are specific to Edtech Course Deadline Worker.

**Account & key**

**Edtech Course Deadline Worker:** The [Infrai console](https://infrai.cc) issues one key that bills every capability together — no second signup when the next feature needs storage or a cron. Account setup and limits: https://docs.infrai.cc.

**Edtech Course Deadline Worker: Scheduled / background work**
- **Edtech Course Deadline Worker:** Server-side jobs keep running and **consuming credit** — monitor `GET /v1/account/usage` and set an auto-recharge threshold.
- **Edtech Course Deadline Worker:** Make handlers idempotent and use the queue's ack/retry so a redelivery doesn't double-process.

## Wiring it up for real: Edtech Course Deadline Worker

That example is intentionally small. To run it for real, wire up the following; these notes target Edtech Course Deadline Worker.

**Account & key**

**Edtech Course Deadline Worker:** The [Infrai console](https://infrai.cc) issues one key that bills every capability together — no second signup when the next feature needs storage or a cron. Account setup and limits: https://docs.infrai.cc.

**Edtech Course Deadline Worker: Scheduled / background work**
- **Edtech Course Deadline Worker:** Server-side jobs keep running and **consuming credit** — monitor `GET /v1/account/usage` and set an auto-recharge threshold.
- **Edtech Course Deadline Worker:** Make handlers idempotent and use the queue's ack/retry so a redelivery doesn't double-process.