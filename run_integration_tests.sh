#!/bin/bash
# Integration test runner for Drakeify proxy
# 
# This script:
# 1. Checks if the proxy is running
# 2. Runs the integration test suite
# 3. Provides helpful output and error messages

set -e

PROXY_URL="http://localhost:8082"
PROXY_CONTAINER="agency-drakeify-1"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "🧪 Drakeify Integration Test Runner"
echo "===================================="
echo ""

# Check if proxy is running
echo -n "Checking if proxy is running... "
if curl -s -f "$PROXY_URL" > /dev/null 2>&1; then
    echo -e "${GREEN}✓${NC}"
else
    echo -e "${RED}✗${NC}"
    echo ""
    echo -e "${YELLOW}Proxy is not running at $PROXY_URL${NC}"
    echo ""
    echo "To start the proxy, run:"
    echo "  docker-compose up -d drakeify"
    echo ""
    echo "Or check the logs:"
    echo "  docker-compose logs drakeify"
    exit 1
fi

# Check if proxy container is healthy
echo -n "Checking proxy container status... "
if docker ps --filter "name=$PROXY_CONTAINER" --filter "status=running" | grep -q "$PROXY_CONTAINER"; then
    echo -e "${GREEN}✓${NC}"
else
    echo -e "${YELLOW}⚠${NC}"
    echo ""
    echo -e "${YELLOW}Warning: Container may not be running properly${NC}"
    echo "Check with: docker-compose ps"
fi

# Check if LLM configs are set up
echo -n "Checking LLM configurations... "
LLM_CONFIGS=$(curl -s -H "Authorization: Bearer change_me_in_production" \
    http://localhost:3974/api/llm/configs 2>/dev/null || echo "[]")

CONFIG_COUNT=$(echo "$LLM_CONFIGS" | jq '. | length' 2>/dev/null || echo "0")

if [ "$CONFIG_COUNT" -gt 0 ]; then
    echo -e "${GREEN}✓ ($CONFIG_COUNT configs)${NC}"
else
    echo -e "${YELLOW}⚠ (no configs)${NC}"
    echo ""
    echo -e "${YELLOW}Warning: No LLM configurations found${NC}"
    echo "The proxy will fall back to environment variables."
    echo "To add configs, use the Web UI at http://localhost:3974"
fi

echo ""
echo "Running integration tests..."
echo "----------------------------"
echo ""

# Run the tests
if cargo test --test proxy_integration_test -- --nocapture --test-threads=1; then
    echo ""
    echo -e "${GREEN}✅ All tests passed!${NC}"
    exit 0
else
    echo ""
    echo -e "${RED}❌ Some tests failed${NC}"
    echo ""
    echo "Troubleshooting:"
    echo "  1. Check proxy logs: docker-compose logs drakeify --tail=50"
    echo "  2. Verify LLM is responding: curl http://localhost:11434/api/tags"
    echo "  3. Check database: docker-compose exec drakeify ls -la /data/"
    exit 1
fi

