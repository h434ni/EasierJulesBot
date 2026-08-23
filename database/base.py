from abc import ABC, abstractmethod
from typing import Optional, Dict, Any, List

class BaseDatabase(ABC):
    @abstractmethod
    async def connect(self):
        pass

    @abstractmethod
    async def disconnect(self):
        pass

    @abstractmethod
    async def get_setting(self, key: str) -> Optional[str]:
        pass

    @abstractmethod
    async def set_setting(self, key: str, value: str):
        pass

    @abstractmethod
    async def create_topic(self, topic_id: int):
        pass

    @abstractmethod
    async def get_topic(self, topic_id: int) -> Optional[Dict[str, Any]]:
        pass

    @abstractmethod
    async def get_topic_by_session(self, session_id: str) -> Optional[Dict[str, Any]]:
        pass

    @abstractmethod
    async def update_topic_session(self, topic_id: int, session_id: str):
        pass

    @abstractmethod
    async def update_topic_auto_pr(self, topic_id: int, auto_pr: bool):
        pass

    @abstractmethod
    async def update_topic_state(self, topic_id: int, state: str):
        pass

    @abstractmethod
    async def delete_topic(self, topic_id: int):
        pass

    @abstractmethod
    async def get_all_active_topics(self) -> List[Dict[str, Any]]:
        pass
