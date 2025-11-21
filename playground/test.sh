#!/bin/bash

# Test script for doty playground
# This script demonstrates how to test different linking scenarios

echo "=== Doty Playground Test Script ==="
echo ""

# Show current directory structure
echo "📁 Current directory structure:"
tree -a
echo ""

# Show configuration file
echo "📄 Configuration file (doty.kdl):"
echo "----------------------------------------"
cat doty.kdl
echo ""

# Example commands to test doty
echo "🚀 Example doty commands to test:"
echo "-----------------------------------"
echo "# From playground directory (no -c flag needed!):"
echo "doty link --dry-run"
echo "doty link"
echo "doty clean"
echo "doty status"
echo ""
echo "# From outside playground directory:"
echo "doty -c playground/doty.kdl link --dry-run"
echo "doty -c playground/doty.kdl link"
echo ""

# Test with different path resolution strategies
echo "🔍 Path Resolution Testing:"
echo "---------------------------"
echo ""
echo "1. With pathResolution=\"config\" (default):"
echo "   - Source paths are relative to playground/ directory"
echo "   - Run from playground/: doty link"
echo "   - Run from anywhere: doty -c playground/doty.kdl link"
echo ""
echo "2. With pathResolution=\"cwd\":"
echo "   - Source paths are relative to current working directory"
echo "   - Edit doty.kdl: change pathResolution to \"cwd\""
echo "   - Run from playground/: doty link"
echo ""

echo "✅ Setup complete! You can now test doty with this configuration."
