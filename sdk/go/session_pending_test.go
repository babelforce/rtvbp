package rtvbp

import (
	"context"
	"errors"
	"sync"
	"testing"
)

func TestPendingResponseAndCancellationHaveExactlyOneWinner(t *testing.T) {
	const iterations = 10_000
	want := pendingResult{response: Response{Payload: []byte(`{"ok":true}`)}}
	for iteration := 0; iteration < iterations; iteration++ {
		session := &Session{pending: make(map[string]*pendingRequest)}
		pending, err := session.registerPending("request")
		if err != nil {
			t.Fatal(err)
		}

		start := make(chan struct{})
		completed := make(chan bool, 1)
		canceled := make(chan bool, 1)
		go func() {
			<-start
			completed <- session.completePending("request", want)
		}()
		go func() {
			<-start
			canceled <- session.cancelPending("request", pending)
		}()
		close(start)

		completionWon := <-completed
		cancellationWon := <-canceled
		if completionWon == cancellationWon {
			t.Fatalf("iteration %d: completion won=%v, cancellation won=%v", iteration, completionWon, cancellationWon)
		}
		if completionWon {
			if got := <-pending.result; string(got.response.Payload) != string(want.response.Payload) || got.err != nil {
				t.Fatalf("iteration %d: result = %#v, want %#v", iteration, got, want)
			}
		} else {
			select {
			case got := <-pending.result:
				t.Fatalf("iteration %d: canceled request received %#v", iteration, got)
			default:
			}
		}
	}
}

func TestPendingFailureAndCancellationHaveExactlyOneWinner(t *testing.T) {
	const iterations = 10_000
	want := errors.New("session failed")
	for iteration := 0; iteration < iterations; iteration++ {
		session := &Session{pending: make(map[string]*pendingRequest)}
		pending, err := session.registerPending("request")
		if err != nil {
			t.Fatal(err)
		}

		start := make(chan struct{})
		failed := make(chan struct{})
		canceled := make(chan bool, 1)
		go func() {
			<-start
			session.failPending(want)
			close(failed)
		}()
		go func() {
			<-start
			canceled <- session.cancelPending("request", pending)
		}()
		close(start)

		cancellationWon := <-canceled
		<-failed
		if cancellationWon {
			select {
			case got := <-pending.result:
				t.Fatalf("iteration %d: canceled request received %#v", iteration, got)
			default:
			}
		} else if got := <-pending.result; !errors.Is(got.err, want) {
			t.Fatalf("iteration %d: failure = %v, want %v", iteration, got.err, want)
		}
	}
}

func TestConcurrentGracefulCloseAndFailuresAreArbitratedBeforeShutdown(t *testing.T) {
	for _, test := range []struct {
		name     string
		failures []stopRequest
		want     []error
	}{
		{
			name:     "keepalive failure upgrades graceful close",
			failures: []stopRequest{{cause: ErrKeepaliveTimeout, failed: true}},
			want:     []error{ErrKeepaliveTimeout},
		},
		{
			name: "reader and send failures are joined with graceful close",
			failures: []stopRequest{
				stopFromTransport(context.Canceled),
				{cause: ErrRequestFailed, failed: true},
			},
			want: []error{context.Canceled, ErrRequestFailed},
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			const iterations = 1_000
			for iteration := 0; iteration < iterations; iteration++ {
				session := &Session{stop: make(chan struct{}, 1)}
				start := make(chan struct{})
				var reports sync.WaitGroup
				reports.Add(1 + len(test.failures))
				go func() {
					defer reports.Done()
					<-start
					session.requestStop(nil, false)
				}()
				for _, failure := range test.failures {
					failure := failure
					go func() {
						defer reports.Done()
						<-start
						session.requestStop(failure.cause, failure.failed)
					}()
				}
				close(start)
				reports.Wait()

				terminal := session.commitStopRequests(stopRequest{})
				if !terminal.failed {
					t.Fatalf("iteration %d: concurrent failure did not upgrade graceful close", iteration)
				}
				for _, want := range test.want {
					if !errors.Is(terminal.cause, want) {
						t.Fatalf("iteration %d: terminal error %v does not include %v", iteration, terminal.cause, want)
					}
				}
			}
		})
	}
}
