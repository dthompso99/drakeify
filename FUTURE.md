The following is just a checklist of future plans, not in any particular order.  This is not a promise of when or if these will be implemented, just a list of ideas that have been bouncing around.  Contributions are welcome!

## Future Plans
 - Remote Agent Support
   - allow tools to run on remote systems, and register themselves with drakeify
 - Micro LLM support
   - embed a small LLM to handle simple tasks, and expose it to the plugin ecosystem.
 - Better Identity Management 
   - allow for multiple identities to be used at once, and for them to be able to call each other
 - Multi-LLM support
   - allow for multiple LLMs to be used at once, and for them to be able to call each other
   - seperate LLMs for images, audio, and text
 - expand testing
   - test with existing agents such as claude code, zed, etc.
   - create a test suite against multiple LLMs to ensure compatibility
 - fine grained permissions
   - while drakeify does not enable any dangerous tools by itself, pluging and tools can.
 - plugin/tool readyness gates
   - check if a plugin/tool requires secrets, config, etc, and if not disable it until those are.
 - a UI
 - a setup wizzard, and quickstart mode.
 - tool_search support:  I think this can be done all via plugins
 - sandbox support: Allow the LLM to run many tool calls via a sandboxed environment.  The tools would need to be specially designed to work in this way.  The LLM would be able to run code, but only in a sandboxed environment.
 - cross-tool calling:  allow tools to call other tools.  This would allow simple tools to be combined into more complex tools.  also a stepping stone for sandboxs.