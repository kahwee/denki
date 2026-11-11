# Offline fixtures

`hs300_hw2_sysinfo.json` preserves the structural edge cases required by the strip
parser: no parent relay state, six short child IDs, mixed power states and the
energy feature flag. Its identity, names, telemetry and firmware build string are
explicitly synthetic. It is not a real diagnostic capture or hardware-verification
evidence. Do not add real device identities, household inventories or captures.
