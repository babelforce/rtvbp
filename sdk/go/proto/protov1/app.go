package protov1

type ApplicationMoveRequest struct {
	ApplicationID string `json:"application_id,omitempty"`
	Continue      bool   `json:"continue,omitempty"`
}

func (m *ApplicationMoveRequest) MethodName() string {
	return "application.move"
}

type ApplicationMoveResponse struct {
}
