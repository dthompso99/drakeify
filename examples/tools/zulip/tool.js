// Zulip Companion Tools
// - list users
// - list topics (for a stream)
// - send message (stream or private)
// Uses existing config + secret layout shown in your plugin snippet:
//   config = get_config("zulip") with { zulip_url, bot_email }
//   secret interpolation: ${secret.zulip.bot_api_key}
// Assumes http.get/http.post exist (like your plugin), and btoa is available.

function _getZulipConfigOrError() {
  const config = get_config("zulip");
  if (!config || !config.zulip_url || !config.bot_email) {
    return {
      ok: false,
      error:
        "Missing Zulip configuration. Please set zulip_url and bot_email in plugin_configs table (config namespace: 'zulip').",
    };
  }
  return { ok: true, config };
}

function _buildAuthHeader(config) {
  // Secret token is interpolated by your HTTP layer
  const auth_string = config.bot_email + ":${secret.zulip.bot_api_key}";
  return "Basic " + btoa(auth_string);
}

function _buildUrlEncodedBody(params) {
  return Object.keys(params)
    .filter((k) => params[k] !== undefined && params[k] !== null)
    .map((k) => encodeURIComponent(k) + "=" + encodeURIComponent(String(params[k])))
    .join("&");
}

// -----------------------
// Tool 1: List Users
// -----------------------
function register_list_users() {
  return {
    name: "zulip_list_users",
    description:
      "List users in the Zulip organization (optionally including inactive bots/users depending on endpoint behavior).",
    parameters: {
      type: "object",
      properties: {
        client_gravatar: {
          type: "boolean",
          description:
            "If true, request gravatar hashes. Defaults to false.",
        },
        include_custom_profile_fields: {
          type: "boolean",
          description:
            "If true, include custom profile fields if supported. Defaults to false.",
        },
      },
      required: [],
    },
  };
}

function execute_list_users(args) {
  const cfg = _getZulipConfigOrError();
  if (!cfg.ok) {
    return JSON.stringify({ success: false, error: cfg.error });
  }

  const config = cfg.config;
  const url =
    config.zulip_url +
    "/api/v1/users" +
    (args && (args.client_gravatar || args.include_custom_profile_fields)
      ? "?" +
        [
          args.client_gravatar ? "client_gravatar=true" : null,
          args.include_custom_profile_fields
            ? "include_custom_profile_fields=true"
            : null,
        ]
          .filter(Boolean)
          .join("&")
      : "");

  const auth_header = _buildAuthHeader(config);

  console.log("[zulip_list_users] GET", url);

  let resp;
  try {
    resp = http.get({
      url: url,
      headers: {
        Authorization: auth_header,
        Accept: "application/json",
      },
    });
  } catch (e) {
    return JSON.stringify({
      success: false,
      error: "HTTP request failed",
      details: String(e),
    });
  }

  if (!resp || resp.success === false) {
    return JSON.stringify({
      success: false,
      error: resp && resp.error ? resp.error : "Zulip API error",
      raw: resp ? resp.data : null,
    });
  }

  try {
    const json = JSON.parse(resp.data);

    // Normalize output: keep it small for the model
    const members = (json.members || []).map((u) => ({
      user_id: u.user_id,
      email: u.email,
      full_name: u.full_name,
      is_bot: !!u.is_bot,
      is_active: u.is_active !== false, // default true if missing
      role: u.role, // may be numeric depending on Zulip version
    }));

    return JSON.stringify({
      success: true,
      count: members.length,
      users: members,
    });
  } catch (e) {
    return JSON.stringify({
      success: false,
      error: "Failed to parse Zulip response",
      details: String(e),
      raw: resp.data,
    });
  }
}

// -----------------------
// Tool 2: List Topics (per stream)
// -----------------------
function register_list_topics() {
  return {
    name: "zulip_list_topics",
    description:
      "List topics for a given Zulip stream (by stream_id).",
    parameters: {
      type: "object",
      properties: {
        stream_id: {
          type: "integer",
          description: "Zulip stream ID (not name).",
        },
      },
      required: ["stream_id"],
    },
  };
}

function execute_list_topics(args) {
  const cfg = _getZulipConfigOrError();
  if (!cfg.ok) {
    return JSON.stringify({ success: false, error: cfg.error });
  }
  const config = cfg.config;

  const stream_id = args.stream_id;
  const url = config.zulip_url + "/api/v1/users/me/" + "subscriptions/" + stream_id + "/topics";

  // Note: Some Zulip deployments use /api/v1/users/me/"streams"/{stream_id}/topics instead.
  // If your server 404s, swap to:
  //   /api/v1/users/me/streams/{stream_id}/topics
  //
  // I kept the most common current pattern above, but you may need to adjust based on server version.

  const auth_header = _buildAuthHeader(config);

  console.log("[zulip_list_topics] GET", url);

  let resp;
  try {
    resp = http.get({
      url: url,
      headers: {
        Authorization: auth_header,
        Accept: "application/json",
      },
    });
  } catch (e) {
    return JSON.stringify({
      success: false,
      error: "HTTP request failed",
      details: String(e),
    });
  }

  if (!resp || resp.success === false) {
    return JSON.stringify({
      success: false,
      error: resp && resp.error ? resp.error : "Zulip API error",
      raw: resp ? resp.data : null,
      hint:
        "If this endpoint 404s on your Zulip version, try /api/v1/users/me/streams/{stream_id}/topics instead.",
    });
  }

  try {
    const json = JSON.parse(resp.data);
    const topics = (json.topics || []).map((t) => ({
      name: t.name,
      max_id: t.max_id,
    }));

    return JSON.stringify({
      success: true,
      stream_id: stream_id,
      count: topics.length,
      topics: topics,
    });
  } catch (e) {
    return JSON.stringify({
      success: false,
      error: "Failed to parse Zulip response",
      details: String(e),
      raw: resp.data,
    });
  }
}

// -----------------------
// Tool 3: Send Message
// -----------------------
function register_send_message() {
  return {
    name: "zulip_send_message",
    description:
      "Send a message via Zulip as the configured bot. Supports stream messages and private messages.",
    parameters: {
      type: "object",
      properties: {
        type: {
          type: "string",
          description: "Message type: 'stream' or 'private'.",
          enum: ["stream", "private"],
        },
        content: {
          type: "string",
          description: "Message content (markdown supported by Zulip).",
        },

        // stream message fields
        stream: {
          type: "string",
          description: "Stream name (required if type='stream').",
        },
        topic: {
          type: "string",
          description: "Topic name (required if type='stream').",
        },

        // private message fields
        to: {
          type: "array",
          description:
            "List of recipient emails (required if type='private').",
          items: { type: "string" },
        },
      },
      required: ["type", "content"],
    },
  };
}

function execute_send_message(args) {
  const cfg = _getZulipConfigOrError();
  if (!cfg.ok) {
    return JSON.stringify({ success: false, error: cfg.error });
  }
  const config = cfg.config;

  // Validate args
  const type = args.type;
  const content = args.content;

  if (type === "stream") {
    if (!args.stream || !args.topic) {
      return JSON.stringify({
        success: false,
        error: "For type='stream', both 'stream' and 'topic' are required.",
      });
    }
  } else if (type === "private") {
    if (!args.to || !Array.isArray(args.to) || args.to.length === 0) {
      return JSON.stringify({
        success: false,
        error: "For type='private', 'to' must be a non-empty array of emails.",
      });
    }
  } else {
    return JSON.stringify({
      success: false,
      error: "Invalid type. Must be 'stream' or 'private'.",
    });
  }

  // Build Zulip message payload as form data
  const form_params = { type: type, content: content };

  if (type === "private") {
    // Zulip expects JSON array string for private message recipients
    form_params.to = JSON.stringify(args.to);
  } else {
    form_params.to = args.stream;
    form_params.topic = args.topic;
  }

  const body = _buildUrlEncodedBody(form_params);

  const api_url = config.zulip_url + "/api/v1/messages";
  const auth_header = _buildAuthHeader(config);

  console.log("[zulip_send_message] POST", api_url, "payload:", JSON.stringify(form_params));

  let resp;
  try {
    resp = http.post({
      url: api_url,
      headers: {
        Authorization: auth_header,
        "Content-Type": "application/x-www-form-urlencoded",
        Accept: "application/json",
      },
      body: body,
    });
  } catch (e) {
    return JSON.stringify({
      success: false,
      error: "HTTP request failed",
      details: String(e),
    });
  }

  if (!resp || resp.success === false) {
    return JSON.stringify({
      success: false,
      error: resp && resp.error ? resp.error : "Zulip API error",
      raw: resp ? resp.data : null,
    });
  }

  try {
    const json = JSON.parse(resp.data);
    // Successful send usually includes { id: <message_id>, ... }
    return JSON.stringify({
      success: true,
      message_id: json.id || null,
      result: json.result || "success",
    });
  } catch (e) {
    // If Zulip returned non-JSON for some reason, still mark success based on transport
    return JSON.stringify({
      success: true,
      warning: "Sent but could not parse response as JSON",
      raw: resp.data,
    });
  }
}

// -----------------------
// Export (multi-tool bundle)
// -----------------------
function register() {
  // Some runtimes only accept a single tool per file. If yours does,
  // split into three files and export each register/execute pair.
  return [
    register_list_users(),
    register_list_topics(),
    register_send_message(),
  ];
}

function execute(args) {
  // If your runtime calls execute() per-tool by name, you won't use this dispatcher.
  // If it calls a single execute() for the bundle, pass args._tool_name to route.
  const toolName = args && args._tool_name;

  if (toolName === "zulip_list_users") return execute_list_users(args);
  if (toolName === "zulip_list_topics") return execute_list_topics(args);
  if (toolName === "zulip_send_message") return execute_send_message(args);

  return JSON.stringify({
    success: false,
    error:
      "Tool dispatcher missing or unknown tool. Provide args._tool_name as one of: zulip_list_users, zulip_list_topics, zulip_send_message.",
  });
}

({ register, execute });