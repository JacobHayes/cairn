# Proof for brief 1.3: Simulation spike: tokio and axum under the shim

tokio, axum, and a hyper client now run under the patina native shim (`testbeds/spike`: one
server, three clients, one current-thread runtime), and injected faults visibly change a run:
requests are retried, retries are deduplicated, and virtual time stretches.

## One seed under each fault

Seed 1, 24 requests per run. Elapsed is virtual time.

| Run | Outcome | Retries | Deduplicated | Elapsed (us) |
|---|---|---|---|---|
| Fault-free | pass | 0 | 0 | 0 |
| Connections reset (10%) | pass | 37 | 11 | 95000 |
| Connections refused (20%) | pass | 5 | 0 | 20000 |
| Segments delayed 2-4 ms, 10% dropped | pass | 0 | 0 | 69000 |
| Responses lost (fault site on every first attempt) | pass | 25 | 24 | 45000 |
| Server that appends a retry again, fault-free | pass | 0 | 0 | 0 |
| The same server, connections reset | `applied-once` violated, 7 requests | 37 | 0 | 95000 |

A refused connect never reaches the server, so nothing is deduplicated; a reset after the server
applied a request makes the retry a duplicate. The double-appending server looks healthy until a
fault forces a retry.

## Known limits

- Only the current-thread runtime with a 1 ms ticker works under the shim; multi-thread runtimes,
  TLS, DNS names, streaming responses, and HTTP/2 are not proven (`testbeds/spike/README.md`,
  Patina gap report).
