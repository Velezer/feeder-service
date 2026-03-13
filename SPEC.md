# SPEC.md — Feeder Service

## Purpose

The feeder-service provides **real-time market trade data** to multiple consumers.  
It ingests trade streams, detects significant trade events, and distributes them efficiently and reliably.

## Goals

- Deliver **real-time trade feeds** for internal services and clients.
- Identify **significant trade spikes**.
- Ensure **reliable delivery** to multiple subscribers.
- Operate with **minimal latency**.


## Core Functionality

- **Ingest Data**: Consume trade streams from external sources.
- **Detect Spikes**: Identify trades exceeding configured thresholds.
  - Ignore invalid spike baselines (zero/negative/non-finite previous price) to avoid noisy or undefined outputs.
- **Broadcast Events**: Distribute filtered events to all subscribers.
- **Log Events**: Maintain records of significant trades for monitoring.
  - Clamp computed processing delay to non-negative values for clock-skew or future timestamp inputs.

- **Telegram Delivery Behavior**: Telegram fanout is optional and non-blocking.
  - If Telegram is disabled or credentials are missing, stream processing remains active without notifier startup.
  - Delivery failures are logged and must not block websocket fanout or signal processing.

