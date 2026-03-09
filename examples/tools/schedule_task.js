// Schedule Task Tool
// Allows the LLM to schedule tasks to run at a specific time

function register() {
    return {
        name: "schedule_task",
        description: "Schedule a task to be executed at a specific time in the future. The task will run with the same context and tools available as the current session.",
        parameters: {
            type: "object",
            properties: {
                prompt: {
                    type: "string",
                    description: "The task to execute when the scheduled time arrives. Be specific about what should be done. Example: 'Check the weather forecast and send me a Zulip message if it will rain tomorrow'"
                },
                run_at: {
                    type: "string",
                    description: "When to run the task. Use ISO 8601 format (e.g., '2026-03-09T08:00:00Z'). Times are in UTC."
                },
                context: {
                    type: "object",
                    description: "Optional additional context for the task",
                    properties: {
                        note: {
                            type: "string",
                            description: "Optional note about this scheduled task"
                        }
                    }
                }
            },
            required: ["prompt", "run_at"]
        }
    };
}

function execute(args) {
    try {
        const prompt = args.prompt;
        const run_at = args.run_at;
        const context = args.context || {};

        // Validate inputs
        if (!prompt || typeof prompt !== 'string') {
            return JSON.stringify({
                success: false,
                error: "prompt is required and must be a string"
            });
        }

        if (!run_at || typeof run_at !== 'string') {
            return JSON.stringify({
                success: false,
                error: "run_at is required and must be a string"
            });
        }

        // Validate ISO 8601 format (basic check)
        const isoRegex = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(Z|[+-]\d{2}:\d{2})$/;
        if (!isoRegex.test(run_at)) {
            return JSON.stringify({
                success: false,
                error: "run_at must be in ISO 8601 format (e.g., '2026-03-09T08:00:00Z')"
            });
        }

        // Check if the time is in the future
        const runAtTime = new Date(run_at).getTime();
        const now = Date.now();
        if (runAtTime <= now) {
            return JSON.stringify({
                success: false,
                error: "run_at must be in the future"
            });
        }

        // Serialize context
        const contextJson = JSON.stringify(context);

        // Call the Rust function to schedule the task
        const resultJson = __rust_schedule_task(prompt, run_at, contextJson);
        const result = JSON.parse(resultJson);

        // Check for error
        if (result.__error) {
            return JSON.stringify({
                success: false,
                error: result.__error
            });
        }

        // Return success with job_id
        return JSON.stringify({
            success: true,
            job_id: result.job_id,
            message: `Task scheduled successfully for ${run_at}`,
            prompt: prompt
        });
    } catch (e) {
        return JSON.stringify({
            success: false,
            error: String(e)
        });
    }
}

// Export the tool
({ register, execute })

