#!/usr/bin/env bash
# Nopipe cluster deploy — nanoclaw (AWS US-East-1, 98.89.6.40)
# Run from hera after contracts are deployed and env is populated

set -e
REMOTE="ubuntu@98.89.6.40"
BINARY="target/release/polyclaw-cluster"
REMOTE_DIR="/opt/nopipe"

echo "=== Building release binary ==="
cd /home/jack/code/polyclaw
cargo build --release -p polyclaw-cluster

echo "=== Uploading binary ==="
ssh $REMOTE "mkdir -p $REMOTE_DIR/bin"
scp $BINARY $REMOTE:$REMOTE_DIR/bin/nopipe-cluster

echo "=== Uploading env template ==="
scp scripts/cluster.env.example $REMOTE:$REMOTE_DIR/.env.example
echo "  → Edit $REMOTE_DIR/.env on nanoclaw before starting"

echo "=== Installing systemd service ==="
scp cluster/polyclaw-cluster.service $REMOTE:/tmp/nopipe-cluster.service
ssh $REMOTE "sudo mv /tmp/nopipe-cluster.service /etc/systemd/system/nopipe-cluster.service && sudo systemctl daemon-reload"

echo ""
echo "=== Next steps on nanoclaw ==="
echo "  1. Edit /opt/nopipe/.env (fill in all vars)"
echo "  2. sudo systemctl start nopipe-cluster"
echo "  3. sudo systemctl enable nopipe-cluster"
echo "  4. curl http://localhost:9000/health"
