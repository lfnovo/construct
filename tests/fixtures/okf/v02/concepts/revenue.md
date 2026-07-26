---
type: Metric
title: Revenue
description: Recognized revenue for a period.
tags: [finance, trusted]
status: stable
stale_after: 2026-12-31
generated:
  by: process:finance
  at: 2026-07-25T12:00:00Z
verified:
  - by: human:luis
    at: 2026-07-25T13:00:00Z
sources:
  - id: policy
    resource: https://example.com/revenue-policy
    title: Revenue policy
domain:
  name: Finance
  critical: true
  owners:
    - team: finance
      priority: 1
---
# Revenue

Revenue belongs to a [customer](./customer.md).
