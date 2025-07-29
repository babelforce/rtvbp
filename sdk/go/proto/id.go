package proto

import gonanoid "github.com/matoous/go-nanoid/v2"

func ID() string {
	i, _ := gonanoid.New()
	return i
}
