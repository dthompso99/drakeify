// DuckDuckGo Search Tool
// Performs a search using DuckDuckGo Instant Answer API

function register() {
    return {
        name: "search_duckduckgo",
        description: "Search DuckDuckGo for information about a topic",
        parameters: {
            type: "object",
            properties: {
                query: {
                    type: "string",
                    description: "Search query"
                }
            },
            required: ["query"]
        }
    };
}

function execute(args) {
    var query = args.query;
    
    console.log("[search_duckduckgo] Searching for:", query);
    
    // Build DuckDuckGo Instant Answer API URL
    var url = "https://api.duckduckgo.com/?format=json&no_html=1&skip_disambig=1&q=" + encodeURIComponent(query);
    
    var result = httpGet(url);
    
    if (!result.success) {
        console.error("[search_duckduckgo] HTTP error:", result.error);
        return JSON.stringify({
            success: false,
            query: query,
            error: result.error
        });
    }
    
    try {
        var json = JSON.parse(result.data);
        
        // Extract useful fields
        var output = {
            success: true,
            query: query,
            abstract: json.Abstract || "",
            abstract_source: json.AbstractSource || "",
            abstract_url: json.AbstractURL || "",
            heading: json.Heading || "",
            answer: json.Answer || "",
            answer_type: json.AnswerType || "",
            definition: json.Definition || "",
            definition_source: json.DefinitionSource || "",
            definition_url: json.DefinitionURL || "",
            related_topics: [],
            results: []
        };
        
        // Related topics
        if (json.RelatedTopics && json.RelatedTopics.length > 0) {
            for (var i = 0; i < json.RelatedTopics.length && output.related_topics.length < 5; i++) {
                var topic = json.RelatedTopics[i];
                
                if (topic.Text && topic.FirstURL) {
                    output.related_topics.push({
                        text: topic.Text,
                        url: topic.FirstURL
                    });
                }
                
                // Handle nested topics
                if (topic.Topics) {
                    for (var j = 0; j < topic.Topics.length && output.related_topics.length < 5; j++) {
                        var sub = topic.Topics[j];
                        if (sub.Text && sub.FirstURL) {
                            output.related_topics.push({
                                text: sub.Text,
                                url: sub.FirstURL
                            });
                        }
                    }
                }
            }
        }
        
        console.log("[search_duckduckgo] Success!");
        
        return JSON.stringify(output);
        
    } catch (err) {
        console.error("[search_duckduckgo] Parse error:", err);
        
        return JSON.stringify({
            success: false,
            query: query,
            error: "Failed to parse DuckDuckGo response",
            raw: result.data
        });
    }
}

// Export the functions
({ register, execute })
