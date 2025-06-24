package audio

type AudioIO interface {
	ReadChan() <-chan []byte
	WriteChan() chan<- []byte
}
