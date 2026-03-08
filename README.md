# Drakeify

## Drakeify is a lightweight agent service that runs between your LLM and the applications that use it.

Out of the box, Drakeify behaves as a smart proxy. It sits in front of your LLM, normalizing model output and restructuring it into reliable, consistent responses for your clients.

Drakeify also provides a plugin and tool system that allows you to extend and customize how your LLM behaves.

Key features:

- **Proxy normalization** – Drakeify cleans up inconsistent LLM responses and converts them into reliable structured output for clients.

- **Plugins** – Plugins can modify the conversation flow, allowing you to inject context, enforce rules, filter responses, or add automation.

- **Webhook bots** – Plugins can expose webhooks, making it easy to build bots that integrate with chat systems like Zulip, Slack, or Discord.

- **Tools** – Tools are small, self-contained plugins that give the LLM new capabilities (running commands, searching systems, sending messages, etc.).

- **Tool registry** – A built-in registry allows tools and plugins to be easily shared and discovered.

- **Transparent proxying** – Drakeify tools remain invisible to the client while still allowing client-side tools to pass through normally.  Agents like copilot and claude can be used, with enhanced capabilities.

for plugin development, see [PLUGINS.md](PLUGINS.md).

for tool development, see [TOOLS.md](TOOLS.md).
