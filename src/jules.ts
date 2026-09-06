import { config } from './config';

export class JulesAPIClient {
  private static BASE_URL = "https://jules.googleapis.com/v1alpha";
  private headers: Record<string, string>;

  constructor(public apiKey: string) {
    this.headers = {
      "x-goog-api-key": this.apiKey,
      "Content-Type": "application/json"
    };
  }

  private async request(method: string, endpoint: string, params?: Record<string, any>, body?: any): Promise<any> {
    let url = new URL(`${JulesAPIClient.BASE_URL}/${endpoint}`);
    
    if (params) {
      for (const [k, v] of Object.entries(params)) {
        if (v !== undefined && v !== null) {
          url.searchParams.append(k, String(v));
        }
      }
    }

    const options: RequestInit = {
      method,
      headers: this.headers,
    };

    if (body) {
      options.body = JSON.stringify(body);
    }

    // Proxy support could be added here with an undici agent if required

    const response = await fetch(url.toString(), options);
    
    if (!response.ok) {
      const text = await response.text();
      throw new Error(`API Error (${response.status}): ${text}`);
    }

    if (method === "DELETE" && [200, 202, 204].includes(response.status)) {
      return {};
    }

    try {
      return await response.json();
    } catch {
      return {};
    }
  }

  async listSources(pageSize = 30, pageToken?: string) {
    return this.request("GET", "sources", { pageSize, pageToken });
  }

  async getSource(sourceId: string) {
    const endpoint = sourceId.startsWith("sources/") ? sourceId : `sources/${sourceId}`;
    return this.request("GET", endpoint);
  }

  async createSession(prompt: string, source: string, branch: string, autoPr: boolean) {
    const payload: any = {
      prompt,
      sourceContext: {
        source,
        githubRepoContext: {
          startingBranch: branch
        }
      }
    };
    if (autoPr) {
      payload.automationMode = "AUTO_CREATE_PR";
    } else {
      payload.requirePlanApproval = false;
    }
    return this.request("POST", "sessions", undefined, payload);
  }

  async getSession(sessionId: string) {
    const endpoint = sessionId.startsWith("sessions/") ? sessionId : `sessions/${sessionId}`;
    return this.request("GET", endpoint);
  }

  async listSessions(pageSize = 30, pageToken?: string) {
    return this.request("GET", "sessions", { pageSize, pageToken });
  }

  async deleteSession(sessionId: string) {
    const endpoint = sessionId.startsWith("sessions/") ? sessionId : `sessions/${sessionId}`;
    return this.request("DELETE", endpoint);
  }

  async sendMessage(sessionId: string, message: string) {
    const endpoint = sessionId.startsWith("sessions/") ? `${sessionId}:sendMessage` : `sessions/${sessionId}:sendMessage`;
    return this.request("POST", endpoint, undefined, { prompt: message });
  }

  async listActivities(sessionId: string, pageSize = 50, pageToken?: string) {
    const endpoint = sessionId.startsWith("sessions/") ? `${sessionId}/activities` : `sessions/${sessionId}/activities`;
    return this.request("GET", endpoint, { pageSize, pageToken });
  }
}
