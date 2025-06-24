package audio

type LoopBack struct {
	c chan []byte
}

func (l *LoopBack) ReadChan() <-chan []byte {
	return l.c
}

func (l *LoopBack) WriteChan() chan<- []byte {
	return l.c
}

func NewLoopBack() *LoopBack {
	return &LoopBack{
		c: make(chan []byte, 1),
	}
}

var _ AudioIO = &LoopBack{}
