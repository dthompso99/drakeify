# Tool Router Plugin

## Overview

The Tool Router plugin intelligently filters tools based on conversation context to reduce LLM context size and improve response quality. It uses keyword matching on tool descriptions to determine which tools are relevant to the current conversation.

## Problem Statement

As more tools are added to the system, the LLM context grows larger, which can:
- Exceed hardware capabilities
- Increase hallucinations
- Slow down response times
- Increase token costs

## Solution

The Tool Router plugin analyzes recent conversation messages and filters the tools array before sending to the LLM, keeping only relevant tools based on detected keywords.

## How It Works

1. **Analyzes Recent Messages**: Examines the last 5 messages in the conversation
2. **Detects Categories**: Matches keywords against predefined categories
3. **Filters Tools**: Keeps only tools whose descriptions match detected categories
4. **Preserves Client Tools**: Always includes essential client tools (e.g., `clear_session`, `get_account_info`)
5. **Conservative Fallback**: If no categories detected, includes all tools

## Keyword Categories

The plugin currently supports these categories:

- **web**: http, url, website, fetch, download, api, request, search, internet, online
- **weather**: weather, temperature, forecast, climate, rain, sunny, cloudy
- **filesystem**: file, directory, folder, path, list, read, write, delete
- **messaging**: zulip, slack, message, chat, send, notification, dm, channel
- **session**: session, clear, reset, conversation, history
- **account**: account, user, profile, info, settings

## Configuration

### Priority
- **Priority**: 15 (runs after `system_prompt_builder` but before `context_pruner`)

### Hooks
- **pre_request**: Filters tools before sending to LLM

## Installation

The plugin is already installed in `data/plugins/tool_router.js`. It will be automatically loaded when the proxy starts.

To disable it, add to your `drakeify.toml`:

```toml
disabled_plugins = ["tool_router"]
```

## Usage

The plugin works automatically. No configuration needed.

### Monitoring

Check the proxy logs to see filtering in action:

```
[Tool Router] Detected categories: weather
[Tool Router] Filtered tools: 7 → 3 (removed 4)
[Tool Router] Kept tools: weather, clear_session, get_account_info
```

## Examples

### Example 1: Weather Query
**User**: "What's the weather like in 90210?"

**Detected Categories**: weather

**Tools Sent to LLM**: 
- `weather`
- `clear_session` (always included)
- `get_account_info` (always included)

### Example 2: Web Search
**User**: "Search for information about Rust programming"

**Detected Categories**: web

**Tools Sent to LLM**:
- `search_duckduckgo`
- `http_get`
- `clear_session` (always included)
- `get_account_info` (always included)

### Example 3: Generic Query
**User**: "Hello, how are you?"

**Detected Categories**: (none)

**Tools Sent to LLM**: All tools (conservative fallback)

## Customization

To customize the keyword categories, edit `data/plugins/tool_router.js`:

```javascript
const keywordCategories = {
    web: ['http', 'url', 'website', ...],
    weather: ['weather', 'temperature', ...],
    // Add your own categories here
    kubernetes: ['pod', 'deployment', 'service', 'kubectl'],
    database: ['sql', 'query', 'table', 'database']
};
```

## Performance Impact

- **Minimal overhead**: Simple keyword matching is very fast
- **Reduces context size**: Can reduce tools from 20+ to 3-5 relevant ones
- **Improves accuracy**: LLM focuses on relevant tools only

## Future Enhancements

- [ ] LLM-based classification for more accurate routing
- [ ] Tool usage statistics to improve keyword matching
- [ ] Dynamic category learning based on tool usage patterns
- [ ] Configuration file for keyword categories
- [ ] Per-user tool preferences

## Testing

Run the test script to verify functionality:

```bash
./test_tool_router.sh
```

This will test various query types and show which tools are filtered.

## Troubleshooting

### All tools are being sent
- Check that keywords are present in your query
- Verify the plugin is loaded (check proxy startup logs)
- Ensure plugin priority is correct (should be 15)

### Important tools are being filtered out
- Add the tool name to the `alwaysInclude` array
- Add relevant keywords to the appropriate category
- Consider lowering the threshold for category detection

## License

Same as Drakeify project.

