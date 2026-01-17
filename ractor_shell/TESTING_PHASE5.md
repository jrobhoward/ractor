# Phase 5 Testing Guide - Remote Connection

This guide shows how to test the remote connection capabilities of the ractor shell.

## Setup

Phase 5 allows the shell to connect to remote ractor_cluster nodes and interact with their actors.

## Prerequisites

- Build the project: `cargo build --all`
- Three terminal windows

## Test Procedure

### Terminal 1: Start node_b (Pong Responder)

```bash
cd /Users/jhoward/git/rust_erlang/ractor_experiments
cargo run --bin node_b
```

You should see:
```
Starting node_b (Pong Responder)
[node_b] Starting NodeServer on port 9002
[node_b] Starting IntrospectionActor for remote shell
[node_b] Starting PingPong actor as RESPONDER
[node_b] Waiting for incoming connections...
```

### Terminal 2: Start node_a (Ping Initiator)

```bash
cd /Users/jhoward/git/rust_erlang/ractor_experiments
cargo run --bin node_a
```

You should see:
```
Starting node_a (Ping Initiator)
[node_a] Starting NodeServer on port 9001
[node_a] Starting IntrospectionActor for remote shell
[node_a] Starting PingPong actor as INITIATOR
[node_a] Connecting to peer at 127.0.0.1:9002
[node_a] Successfully connected to 127.0.0.1:9002
```

The nodes will exchange 10 ping/pong messages and then shut down. **Keep them running** by commenting out the shutdown logic or waiting to start them until the shell is ready.

### Terminal 3: Start the Shell and Connect

```bash
cd /Users/jhoward/git/rust_erlang/ractor_experiments/ractor_shell
cargo run
```

## Shell Test Commands

Once the shell starts, try these commands:

### 1. Check Local State

```
ractor@local > help
ractor@local > actors
ractor@local > registry
```

### 2. Connect to node_b

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
```

### 3. List Connected Nodes

```
ractor@local > nodes
```

### 4. Switch to Remote Node Context

```
ractor@local > use 127.0.0.1:9002
```

The prompt should change to show you're now working with the remote node.

### 5. Query Remote Actors

```
ractor@127.0.0.1:9002 > registry
ractor@127.0.0.1:9002 > actors
ractor@127.0.0.1:9002 > pg members ping_pong
ractor@127.0.0.1:9002 > info ping_pong
ractor@127.0.0.1:9002 > info introspection
```

### 6. Connect to node_a as Well

```
ractor@127.0.0.1:9002 > use local
ractor@local > connect 127.0.0.1:9001
ractor@local > nodes
ractor@local > use 127.0.0.1:9001
ractor@127.0.0.1:9001 > registry
ractor@127.0.0.1:9001 > actors
```

### 7. Switch Between Nodes

```
ractor@127.0.0.1:9001 > use 127.0.0.1:9002
ractor@127.0.0.1:9002 > pg members ping_pong
ractor@127.0.0.1:9002 > use local
ractor@local > registry
```

### 8. Disconnect and Exit

```
ractor@local > disconnect 127.0.0.1:9002
ractor@local > nodes
ractor@local > exit
```

## Expected Behavior

### Connection
- Shell spawns a NodeServer on port 9100 on first connect
- Connects to remote node using ractor_cluster client_connect
- Discovers IntrospectionActor via well-known process group
- Tests connection with a Ping RPC

### Remote Queries
- All registry, actors, pg members, and info commands work on remote nodes
- Results show data from the remote node, not local
- Prompt indicates current node context

### Node Switching
- `use <node>` switches context without disconnecting
- `use local` returns to local context
- `nodes` command shows all connected nodes with * for current

## Troubleshooting

### "No introspection actor found on remote node"
- Make sure the target node has spawned an IntrospectionActor
- Check that it joins the "ractor_shell_introspection" process group
- Verify the node is running with the same cookie ("secret_cookie")

### "Connection timeout"
- Ensure the target node's NodeServer is running and listening
- Check that ports are not blocked by firewall
- Verify you're using the correct host:port

### "Not connected to node 'X'"
- Use `nodes` command to see connected nodes
- Make sure you're using the exact host:port string from the connect command
- Try reconnecting if the connection was lost

## Phase 5 Complete!

You've successfully:
- ✓ Connected the shell to remote ractor_cluster nodes
- ✓ Queried remote actors via RPC to IntrospectionActor
- ✓ Switched between multiple node contexts
- ✓ Used the same commands locally and remotely

Next phases will add:
- Phase 6: Mesh topology awareness
- Phase 7: UX polish (tab completion, syntax highlighting)
