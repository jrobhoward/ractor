# Phase 6 Testing Guide - Mesh Topology Awareness

This guide shows how to test the cluster topology features of the ractor shell.

## What's New in Phase 6

Phase 6 adds full cluster topology awareness:
- **Automatic topology discovery** when connecting to a node
- **`cluster` command** with subcommands to view the mesh
- **Cross-cluster visibility** of nodes, process groups, and actors
- **Topology caching** for quick access

## Prerequisites

- Build the project: `cargo build --all`
- Three terminal windows

## Test Procedure

### Terminal 1: Start node_b (Responder Node)

```bash
cd /Users/jhoward/git/rust_erlang/ractor_experiments
cargo run --bin node_b
```

You should see it start up and spawn IntrospectionActor and PingPongActor. **Leave this running.**

### Terminal 2: Start node_a (Initiator Node)

```bash
cd /Users/jhoward/git/rust_erlang/ractor_experiments
cargo run --bin node_a
```

This will connect to node_b and they'll exchange ping/pong messages. **Keep them running** for topology testing.

### Terminal 3: Start the Shell and Test Topology Features

```bash
cd /Users/jhoward/git/rust_erlang/ractor_experiments/ractor_shell
cargo run
```

## Shell Test Commands

### 1. Basic Connection (Same as Phase 5)

```
ractor@local > connect 127.0.0.1:9002
```

You should see:
```
Connecting to 127.0.0.1:9002
  Starting local NodeServer...
  ✓ NodeServer started on port 9100
  ✓ Connected to 127.0.0.1:9002
  Discovering introspection actor...
  ✓ pong from node_b
✓ Connected to node at 127.0.0.1:9002
  Discovering cluster topology...
  ✓ Discovered 2 nodes and 2 process groups
```

Notice the automatic topology discovery at the end!

### 2. View Cluster Nodes

```
ractor@local > cluster
```

or

```
ractor@local > cluster nodes
```

Expected output:
```
Fetching cluster topology...

Cluster Nodes (2):

+---------+-----------+--------+-------+
| Node ID | Node Name | Actors | Local |
+---------+-----------+--------+-------+
| 1       | node_b    | 2      | yes   |
| 2       | node_a    | 0      | no    |
+---------+-----------+--------+-------+
```

### 3. View Process Groups Across Cluster

```
ractor@local > cluster groups
```

Expected output:
```
Fetching cluster topology...

Process Groups (2):

Group: ping_pong
+----------+-------------+--------------+
| Actor ID | Name        | Node         |
+----------+-------------+--------------+
| 1.0      | ping_pong   | node_b (1)   |
| 2.0      | ping_pong   | node_a (2)   |
+----------+-------------+--------------+

Group: ractor_shell_introspection
+----------+----------------+--------------+
| Actor ID | Name           | Node         |
+----------+----------------+--------------+
| 1.1      | introspection  | node_b (1)   |
| 2.1      | introspection  | node_a (2)   |
+----------+----------------+--------------+
```

### 4. View All Actors Across Cluster

```
ractor@local > cluster actors
```

Expected output:
```
Fetching cluster topology...

Actors Across Cluster:

+----------+----------------+--------------+
| Actor ID | Name           | Node         |
+----------+----------------+--------------+
| 1.0      | ping_pong      | node_b (1)   |
| 1.1      | introspection  | node_b (1)   |
| 2.0      | ping_pong      | node_a (2)   |
| 2.1      | introspection  | node_a (2)   |
+----------+----------------+--------------+

Total: 4 actors
```

### 5. Connect to Second Node and View Topology

```
ractor@local > connect 127.0.0.1:9001
ractor@local > nodes
ractor@local > cluster
```

You should see the same topology from either node's perspective!

### 6. Compare with Individual Node Queries

Switch to a specific node and compare:

```
ractor@local > use 127.0.0.1:9002
ractor@127.0.0.1:9002 > registry
ractor@127.0.0.1:9002 > pg members ping_pong
ractor@127.0.0.1:9002 > use local
ractor@local > cluster groups
```

Notice that:
- `registry` shows only local actors
- `pg members` shows actors from all nodes in that group
- `cluster groups` shows all groups with all their members

## Key Features Demonstrated

### Automatic Topology Discovery
- Shell automatically discovers the cluster when connecting
- Shows node count and process group count
- Caches topology for quick access

### Cluster-Wide Visibility
- See all nodes in the cluster, even ones you're not directly connected to
- View which actors are on which nodes
- See process groups spanning multiple nodes

### Multiple Subcommands
- `cluster` or `cluster nodes` - Show all nodes
- `cluster groups` - Show all process groups with members
- `cluster actors` - Show all actors across the cluster

### Topology Inference
- Discovers nodes by examining actor IDs (e.g., "1.0" = node 1, actor 0)
- Extracts node information from process group memberships
- No need for explicit node list API (works with ractor 0.15)

## Comparison: Phase 5 vs Phase 6

### Phase 5 (Remote Connection)
```
ractor@local > connect 127.0.0.1:9002
ractor@local > use 127.0.0.1:9002
ractor@127.0.0.1:9002 > registry    # Shows only this node's actors
ractor@127.0.0.1:9002 > pg members ping_pong  # Shows group members (all nodes)
```

### Phase 6 (Mesh Topology)
```
ractor@local > connect 127.0.0.1:9002
ractor@local > cluster              # Shows ALL nodes at once
ractor@local > cluster groups       # Shows ALL groups with ALL members
ractor@local > cluster actors       # Shows ALL actors in cluster
```

Phase 6 gives you a bird's-eye view of the entire cluster!

## Troubleshooting

### "Not connected to any nodes"
- Use `connect <host:port>` first before running cluster commands
- The cluster command requires at least one active connection

### Empty or partial topology
- Make sure both nodes are running and connected to each other
- Check that IntrospectionActors are spawned on all nodes
- Verify actors are joining well-known process groups

### Node names show as "node_1", "node_2"
- This is expected! Node names are inferred from actor IDs
- The local node shows its actual name (e.g., "node_b")
- Remote nodes show generic names unless we can extract better information

## Phase 6 Complete!

You've successfully:
- ✓ Automatically discovered cluster topology on connect
- ✓ Viewed all nodes in the cluster with the `cluster` command
- ✓ Inspected process groups across the entire mesh
- ✓ Listed all actors cluster-wide
- ✓ Gained full visibility into the distributed system

Next phases could add:
- Phase 7: UX polish (tab completion, syntax highlighting)
- Phase 2: Supervision trees and enhanced introspection
- Phase 3: Monitoring and live event streams
