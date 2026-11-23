Write a security advisory investigation of {name}.

Subject: {name}
Advisory source: {address}
Where this research came from: {origin}

Its shape is the rulebook bmo-research-security-advisories's. Read the vendor's advisory and fix notices, the CVE record and the distributions' security lists yourself. Read writeups of how the flaw is exploited only as far as the exposure judgement needs.

Keep these sections in the body, under these names exactly.

- `Flaw summary` — what is wrong and what it affects.
- `Affected versions` — the range exactly as the vendor states it.
- `Exposure of our platform` — the judgement against the stack inventory below. Write it as one table, one row for each component of the inventory. The table's header row is exactly these five columns, and the Touched cell opens with `Yes` or `No`.

  | Component | Version | Repository | Host | Touched |
- `Fixed version and patch steps` — the vendor's fixed version, and the steps to take wherever it touches.

Our platform's stack inventory:

{stack}

The platform's age floor, the oldest recorded component these runs are judged against:

{floor}
