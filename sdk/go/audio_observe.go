package rtvbp

type AudioStreamObserver struct {
	OnRead          func(n int)
	OnReadError     func(err error)
	OnWrite         func(n int)
	OnWriteError    func(err error)
	OnBufferCleared func(n int)
}

type ObservableAudio struct {
	s  *Session
	o  AudioStreamObserver
	ha HandlerAudio
}

func (s *ObservableAudio) Read(p []byte) (n int, err error) {
	n, err = s.ha.Read(p)
	if err != nil && s.o.OnReadError != nil {
		s.o.OnReadError(err)
	} else if s.o.OnRead != nil {
		s.o.OnRead(n)
	}
	return n, err
}

func (s *ObservableAudio) Write(p []byte) (n int, err error) {
	n, err = s.ha.Write(p)
	if err != nil && s.o.OnWriteError != nil {
		s.o.OnWriteError(err)
	} else if s.o.OnWrite != nil {
		s.o.OnWrite(n)
	}
	return n, err
}

func (s *ObservableAudio) ClearReadBuffer() (int, error) {
	n, err := s.ha.ClearReadBuffer()
	if err == nil && s.o.OnBufferCleared != nil {
		s.o.OnBufferCleared(n)
	}
	return n, err
}

var _ HandlerAudio = &ObservableAudio{}
