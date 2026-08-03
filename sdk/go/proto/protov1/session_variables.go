package protov1

type SessionSetRequest struct {
	Data map[string]any `json:"data"`
}

func (s *SessionSetRequest) MethodName() string {
	return "session.set"
}

type SessionGetRequest struct {
	Keys []string `json:"keys"`
}

func (s *SessionGetRequest) MethodName() string {
	return "session.get"
}
