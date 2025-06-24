package audio

type Io interface {
	ReadChan() <-chan []byte
	WriteChan() chan<- []byte
}

type audioIO struct {
	r <-chan []byte
	w chan<- []byte
}

func (a *audioIO) ReadChan() <-chan []byte {
	return a.r
}

func (a *audioIO) WriteChan() chan<- []byte {
	return a.w
}

func NewIO(r <-chan []byte, w chan<- []byte) Io {
	return &audioIO{r, w}
}
