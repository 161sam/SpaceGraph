watch:
  paths:
    - /etc
    - /home
  ignore:
    - /home/*/.cache
rate_limits:
  max_events_per_sec: 200
  coalesce_window_ms: 250
privacy:
  redact_cmdline: true
  hash_paths: false
