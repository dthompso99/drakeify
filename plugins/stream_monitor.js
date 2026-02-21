// Stream Monitor Plugin
// Monitors streaming chunks in real-time

function register() {
    return {
        name: "stream_monitor",
        description: "Monitors and can modify streaming chunks in real-time",
        priority: 50, // Run in the middle
        hooks: {
            on_stream_chunk: true
        }
    };
}

function on_stream_chunk(data) {
    // data: { chunk, accumulated, chunk_index }
    
    // Example: Could modify chunks in real-time
    // For now, just pass through unchanged
    
    // In a real implementation, you might:
    // - Filter out unwanted patterns
    // - Add formatting
    // - Track streaming statistics
    // - Detect and handle streaming errors
    
    return {
        chunk: data.chunk,
        accumulated: data.accumulated,
        chunk_index: data.chunk_index
    };
}

// Export the functions
({ register, on_stream_chunk })

