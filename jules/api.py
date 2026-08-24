import os
import aiohttp
from typing import Dict, Any, List, Optional

class JulesAPIClient:
    BASE_URL = "https://jules.googleapis.com/v1alpha"

    def __init__(self, api_key: str):
        self.api_key = api_key
        self.headers = {
            "x-goog-api-key": self.api_key,
            "Content-Type": "application/json"
        }
        self.session = None

    def _get_connector(self):
        proxy_url = os.environ.get("PROXY")
        if proxy_url:
            from aiohttp_socks import ProxyConnector
            return ProxyConnector.from_url(proxy_url)
        return None

    async def __aenter__(self):
        self.session = aiohttp.ClientSession(headers=self.headers, connector=self._get_connector())
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        if self.session:
            await self.session.close()

    async def _request(self, method: str, endpoint: str, **kwargs) -> Dict[str, Any]:
        url = f"{self.BASE_URL}/{endpoint}"
        if not self.session:
            async with aiohttp.ClientSession(headers=self.headers, connector=self._get_connector()) as session:
                return await self._do_request(session, method, url, **kwargs)
        else:
            return await self._do_request(self.session, method, url, **kwargs)

    async def _do_request(self, session, method: str, url: str, **kwargs) -> Dict[str, Any]:
        async with session.request(method, url, **kwargs) as response:
            if response.status >= 400:
                text = await response.text()
                raise Exception(f"API Error ({response.status}): {text}")

            if method == "DELETE" and response.status in (200, 204, 202):
                return {}

            try:
                return await response.json()
            except Exception:
                return {}

    async def list_sources(self, page_size: int = 30, page_token: Optional[str] = None) -> Dict[str, Any]:
        params = {"pageSize": page_size}
        if page_token:
            params["pageToken"] = page_token
        return await self._request("GET", "sources", params=params)

    async def get_source(self, source_id: str) -> Dict[str, Any]:
        if source_id.startswith("sources/"):
            endpoint = source_id
        else:
            endpoint = f"sources/{source_id}"
        return await self._request("GET", endpoint)

    async def create_session(self, prompt: str, source: str, branch: str, auto_pr: bool) -> Dict[str, Any]:
        payload = {
            "prompt": prompt,
            "sourceContext": {
                "source": source,
                "githubRepoContext": {
                    "startingBranch": branch
                }
            }
        }
        if auto_pr:
            payload["automationMode"] = "AUTO_CREATE_PR"
        else:
            payload["requirePlanApproval"] = False

        return await self._request("POST", "sessions", json=payload)

    async def get_session(self, session_id: str) -> Dict[str, Any]:
        if session_id.startswith("sessions/"):
            endpoint = session_id
        else:
            endpoint = f"sessions/{session_id}"
        return await self._request("GET", endpoint)

    async def list_sessions(self, page_size: int = 30, page_token: Optional[str] = None) -> Dict[str, Any]:
        params = {"pageSize": page_size}
        if page_token:
            params["pageToken"] = page_token
        return await self._request("GET", "sessions", params=params)

    async def delete_session(self, session_id: str):
        if session_id.startswith("sessions/"):
            endpoint = session_id
        else:
            endpoint = f"sessions/{session_id}"
        return await self._request("DELETE", endpoint)

    async def send_message(self, session_id: str, message: str):
        if session_id.startswith("sessions/"):
            endpoint = f"{session_id}:sendMessage"
        else:
            endpoint = f"sessions/{session_id}:sendMessage"
        return await self._request("POST", endpoint, json={"prompt": message})

    async def list_activities(self, session_id: str, page_size: int = 50, page_token: Optional[str] = None) -> Dict[str, Any]:
        if session_id.startswith("sessions/"):
            endpoint = f"{session_id}/activities"
        else:
            endpoint = f"sessions/{session_id}/activities"

        params = {"pageSize": page_size}
        if page_token:
            params["pageToken"] = page_token

        return await self._request("GET", endpoint, params=params)
