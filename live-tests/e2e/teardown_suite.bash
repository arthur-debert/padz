#!/usr/bin/env bash

# This file is sourced by BATS after running the test suite
# It cleans up any resources that were set up

teardown_suite() {
    echo "🧹 E2E Test Suite Cleanup Complete"
}