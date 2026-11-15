from models import User

DB: dict[str, User] = {
    "alice": User("alice", "secret123"),
}

def get_user(username: str) -> User | None:
    return DB.get(username)

