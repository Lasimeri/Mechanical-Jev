# phi.rs: Intel Phi Jev's server on demand

`ensure` asks `GET /health`; if the server is on this machine
(`127.0.0.1` or `localhost`) and does not answer, it runs Intel Phi Jev's
`xks serve --detach --bind host:port`, which loads the subject and site
from that repository's `xks.conf`, puts the Phi cards to work, and returns
once the server answers (about 45 s for the 35B). A remote server that does
not answer is an error, never started. `MJEV_AUTOSTART=0` turns the
starting off. `stop` runs `xks stop`, which ends the server and releases
the cards' huge pages.

Measured 2026-09-24, from a stopped server: `mjev query` of
examples/query.json took 47 s end to end (start, weight upload to the
cards, the first request 19.5 s); the next single question took 1.8 s,
served on the cards site (devices Phi and CPU).
