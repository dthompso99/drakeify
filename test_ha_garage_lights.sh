#!/bin/bash

# Test script for Home Assistant - Find and control garage shop lights
# This demonstrates the full workflow of discovering and controlling entities

set -e

PROXY_URL="http://localhost:8082"
AUTH_TOKEN="change_me_in_production"

echo "=========================================="
echo "Home Assistant - Garage Shop Lights Test"
echo "=========================================="
echo ""

# Step 1: Find garage shop switch
echo "1. Finding garage shop switch..."
echo "   Using ha_list_states with domain=switch and search=shop"
echo ""

cat > /tmp/ha_find_switch.json <<'EOF'
{
  "model": "qwen3-coder:30b",
  "messages": [
    {
      "role": "user",
      "content": "Use the ha_list_states tool to find switches with 'shop' in the name. Use domain='switch' and search='shop'."
    }
  ],
  "stream": false
}
EOF

echo "   Sending request to find switches..."
curl -s -X POST "${PROXY_URL}/v1/chat/completions" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -d @/tmp/ha_find_switch.json > /tmp/ha_find_response.json

echo "   Response saved to /tmp/ha_find_response.json"
echo ""

# Extract the response content
echo "2. LLM Response:"
cat /tmp/ha_find_response.json | jq -r '.choices[0].message.content' 2>/dev/null || echo "   (Could not parse response)"
echo ""

# Step 2: Turn on the switch
echo "3. Turning ON shop light switch..."
echo ""

cat > /tmp/ha_turn_on.json <<'EOF'
{
  "model": "qwen3-coder:30b",
  "messages": [
    {
      "role": "user",
      "content": "Use the ha_call_service tool to turn on the switch entity 'switch.shop_light'. Use domain='switch', service='turn_on', and entity_id='switch.shop_light'."
    }
  ],
  "stream": false
}
EOF

echo "   Sending request to turn on switch..."
curl -s -X POST "${PROXY_URL}/v1/chat/completions" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -d @/tmp/ha_turn_on.json > /tmp/ha_turn_on_response.json

echo "   Response saved to /tmp/ha_turn_on_response.json"
echo ""

echo "4. LLM Response:"
cat /tmp/ha_turn_on_response.json | jq -r '.choices[0].message.content' 2>/dev/null || echo "   (Could not parse response)"
echo ""

# Wait a moment
echo "5. Waiting 3 seconds..."
sleep 3
echo ""

# Step 3: Turn off the switch
echo "6. Turning OFF shop light switch..."
echo ""

cat > /tmp/ha_turn_off.json <<'EOF'
{
  "model": "qwen3-coder:30b",
  "messages": [
    {
      "role": "user",
      "content": "Use the ha_call_service tool to turn off the switch entity 'switch.shop_light'. Use domain='switch', service='turn_off', and entity_id='switch.shop_light'."
    }
  ],
  "stream": false
}
EOF

echo "   Sending request to turn off switch..."
curl -s -X POST "${PROXY_URL}/v1/chat/completions" \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer ${AUTH_TOKEN}" \
    -d @/tmp/ha_turn_off.json > /tmp/ha_turn_off_response.json

echo "   Response saved to /tmp/ha_turn_off_response.json"
echo ""

echo "7. LLM Response:"
cat /tmp/ha_turn_off_response.json | jq -r '.choices[0].message.content' 2>/dev/null || echo "   (Could not parse response)"
echo ""

# Check logs
echo "8. Checking recent logs for [home_assistant] entries..."
docker logs agency-drakeify-1 2>&1 | grep "\[home_assistant\]" | tail -50 || echo "   No logs found"
echo ""

echo "=========================================="
echo "Test complete!"
echo "=========================================="
echo ""
echo "Files created:"
echo "  /tmp/ha_find_response.json - Response from finding lights"
echo "  /tmp/ha_turn_on_response.json - Response from turning on"
echo "  /tmp/ha_turn_off_response.json - Response from turning off"
echo ""
echo "To view full responses:"
echo "  cat /tmp/ha_find_response.json | jq ."
echo "  cat /tmp/ha_turn_on_response.json | jq ."
echo "  cat /tmp/ha_turn_off_response.json | jq ."

