# Example Plugin

This is a simple example plugin that demonstrates the plugin structure for Drakeify.

## Features

- Logs pre-request information (number of messages and tools)
- Logs post-response information (content length and tool calls)
- Does not modify any data, just observes

## Usage

This plugin is automatically loaded when installed in the `plugins/` directory.

## Hooks

- `pre_request`: Called before sending a request to the LLM
- `post_response`: Called after receiving a response from the LLM

