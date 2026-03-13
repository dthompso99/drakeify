#!/bin/bash

# Test script for Home Assistant list_states tool

echo "=========================================="
echo "Home Assistant - List States Test"
echo "=========================================="
echo ""

# Test by listing all states
echo "1. Testing ha_list_states tool..."
echo "   This will list all entities in Home Assistant"
echo ""

# Create a test payload that will call the tool
cat > /tmp/ha_list_test.json <<'EOF'
{
  "model": "qwen3-coder:30b",
  "messages": [
    {
      "role": "user",
      "content": "Use the ha_list_states tool to list all available entities in Home Assistant"
    }
  ],
  "stream": false,
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "ha_list_states",
        "description": "List all entities or filter by domain/state",
        "parameters": {
          "type": "object",
          "properties": {
            "domain": {
              "type": "string",
              "description": "Optional domain filter (e.g., 'light', 'switch', 'sensor')"
            },
            "state": {
              "type": "string",
              "description": "Optional state filter (e.g., 'on', 'off')"
            }
          }
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
    -d @/tmp/ha_list_test.json > /tmp/ha_list_response.json

echo "   Response saved to /tmp/ha_list_response.json"
echo ""

# Check logs for debug output
echo "2. Checking logs for [home_assistant] debug output..."
echo "   (Waiting 2 seconds for logs to flush...)"
sleep 2
docker logs agency-drakeify-1 2>&1 | grep "\[home_assistant\]" | tail -100 || echo "   No [home_assistant] logs found"
echo ""

echo "=========================================="
echo "Test complete!"
echo "=========================================="
echo ""
echo "To view the full response:"
echo "  cat /tmp/ha_list_response.json | jq ."
echo ""
echo "To view just the tool result:"
echo "  cat /tmp/ha_list_response.json | jq '.choices[0].message.content'"

