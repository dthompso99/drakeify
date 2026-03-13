#!/bin/bash

# Test script for Home Assistant tool
# This script tests the home_assistant tool by calling it through the proxy

echo "=========================================="
echo "Home Assistant Tool Test"
echo "=========================================="
echo ""

# Check if container is running
echo "1. Checking if drakeify container is running..."
if ! docker ps | grep -q "agency-drakeify-1"; then
    echo "❌ Drakeify container is not running"
    echo "   Start it with: docker-compose up -d drakeify"
    exit 1
fi
echo "✅ Container is running"
echo ""

# Test by making a direct tool call through the proxy
# We'll use a simple test that should trigger the debug logging
echo "2. Testing ha_get_state tool..."
echo "   This will trigger debug logging in the container"
echo ""

# Create a test payload that will call the tool
cat > /tmp/ha_test.json <<'EOF'
{
  "model": "qwen3-coder:30b",
  "messages": [
    {
      "role": "user",
      "content": "Use the ha_get_state tool to get the state of entity light.living_room"
    }
  ],
  "stream": false,
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "ha_get_state",
        "description": "Get the current state of a Home Assistant entity",
        "parameters": {
          "type": "object",
          "properties": {
            "entity_id": {
              "type": "string",
              "description": "The entity ID to query (e.g., light.living_room)"
            }
          },
          "required": ["entity_id"]
        }
      }
    }
  ]
}
EOF

echo "   Sending request to proxy..."
curl -s -X POST "http://localhost:8082/v1/chat/completions" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer change_me_in_production" \
    -d @/tmp/ha_test.json > /tmp/ha_response.json

echo "   Response saved to /tmp/ha_response.json"
echo ""

# Check logs for debug output
echo "3. Checking logs for [home_assistant] debug output..."
echo "   (Waiting 2 seconds for logs to flush...)"
sleep 2
docker logs agency-drakeify-1 2>&1 | grep "\[home_assistant\]" | tail -50 || echo "   No [home_assistant] logs found"
echo ""

echo "=========================================="
echo "Test complete!"
echo "=========================================="
echo ""
echo "To view the full response:"
echo "  cat /tmp/ha_response.json | jq ."
echo ""
echo "To view all container logs:"
echo "  docker logs agency-drakeify-1 2>&1 | tail -100"

