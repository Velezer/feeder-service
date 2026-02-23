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
- **Broadcast Events**: Distribute filtered events to all subscribers.
- **Log Events**: Maintain records of significant trades for monitoring.
