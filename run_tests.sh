#!/bin/bash
cd /Users/adebert/h/padz
echo "Running TestShouldRunViewOrOpen..."
go test -v ./cmd/padz/cli -run TestShouldRunViewOrOpen
echo ""
echo "Running TestShouldRunCreate..."
go test -v ./cmd/padz/cli -run TestShouldRunCreate