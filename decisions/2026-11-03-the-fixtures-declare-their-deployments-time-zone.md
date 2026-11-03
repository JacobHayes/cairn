# The fixtures declare their deployment's time zone

Question: a time of day is stored with the deployment's offset at that moment, and the service checks the offset on write. The fixtures are a record of one deployment, so which zone are their times written in, and where is it said?

Call: `fixtures/deployment.yaml` declares the time zone, `Etc/GMT+5`: five hours behind UTC all year, so code that assumes UTC fails in every test and in the browser. Every deployment that loads the fixtures takes its zone from that file: `cairn demo`, the binary's tests that seed them, every integration harness, and the durability testbed. A host resolves the declared name with its own zone database (the system's in the binary, jiff's bundled one in tests and the testbed); the service reads the name through `cairn_service::time_zone` and plays the fixtures with `cairn_service::play`. A time of day in a fixture is written with the zone's offset (`-05:00`), so the vendor evaluation's last step carries a pin, a date answer and a snooze with times, and the screenshot walk sets none of its own.

Alternatives: UTC for the demo and a zone per harness (the fixtures could then hold no time of day, and the walk would patch times in); stamping the demo's offset onto fixture times at load (a translation step between the fixtures and the deployment); a per-test zone override.

What would change it: a second set of fixtures that records a deployment in another zone.
