// Hallucination Corrector Plugin
// Detects and corrects common LLM hallucinations

function register() {
    return {
        name: "hallucination_corrector",
        description: "Detects and corrects common LLM hallucinations in responses",
        priority: 10, // Run very early, before tool execution
        hooks: {
            on_llm_response: true
        }
    };
}

function on_llm_response(data) {
    // data: { content, tool_calls }

    let corrected_content = data.content;
    let corrected_tool_calls = data.tool_calls || [];

    // Detect if the assistant hallucinated a tool call as text content
    if (corrected_content && typeof corrected_content === 'string') {
        // First, fix common Python-style boolean values
        let fixedContent = corrected_content
            .replace(/:\s*True\b/g, ': true')
            .replace(/:\s*False\b/g, ': false')
            .replace(/:\s*None\b/g, ': null');

        // Strategy 1: Try parsing the entire content as JSON
        let parsed = null;
        let prefixText = "";

        try {
            parsed = JSON.parse(fixedContent);
        } catch (e) {
            // Strategy 2: Look for JSON object embedded in text
            // Find the first '{' and try to extract a valid JSON object from there
            const firstBrace = fixedContent.indexOf('{');
            if (firstBrace !== -1) {
                // Extract everything before the JSON as prefix text
                prefixText = fixedContent.substring(0, firstBrace).trim();

                // Try to find matching closing brace by parsing incrementally
                let jsonStr = null;
                for (let i = fixedContent.length; i > firstBrace; i--) {
                    const candidate = fixedContent.substring(firstBrace, i);
                    try {
                        const testParsed = JSON.parse(candidate);
                        // Successfully parsed - check if it has the right structure
                        if (testParsed && typeof testParsed === 'object' && testParsed.name && testParsed.parameters) {
                            parsed = testParsed;
                            jsonStr = candidate;
                            break;
                        }
                    } catch (parseError) {
                        // Keep trying with shorter strings
                        continue;
                    }
                }
            }
        }

        // Check if we found a hallucinated tool call
        if (parsed && typeof parsed === 'object' && parsed.name && parsed.parameters) {
            const toolName = parsed.name;
            const parameters = parsed.parameters;

            // Create a proper tool call in Ollama format
            const toolCall = {
                id: `call_${Date.now()}`,
                function: {
                    name: toolName,
                    arguments: parameters
                }
            };

            // Add to tool_calls array
            corrected_tool_calls.push(toolCall);

            // Keep any prefix text (like "Let me try again") but remove the JSON
            corrected_content = prefixText;

            console.log(`[hallucination_corrector] Converted hallucinated tool call: ${toolName}`);
        }
    }

    // Remove phrases like "I don't have access to real-time data" when we actually do have tools
    if (corrected_tool_calls && corrected_tool_calls.length > 0) {
        corrected_content = corrected_content ? corrected_content.replace(
            /I don't have access to real-time (data|information)/gi,
            "Based on the available tools"
        ) : corrected_content;
    }

    // Return modified data
    return {
        content: corrected_content,
        tool_calls: corrected_tool_calls
    };
}

// Export the functions
({ register, on_llm_response })

