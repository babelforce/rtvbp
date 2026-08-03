// Package babelforceauth demonstrates authentication for inbound babelforce Cloud RTVBP sessions.
// It is deployment-specific example code, not part of the protocol runtime.
package babelforceauth

import (
	"crypto/rsa"
	"fmt"
	"net/http"
	"strings"

	"github.com/golang-jwt/jwt/v5"
)

const (
	Issuer       = "auth.babelforce.com"
	Subject      = "com.babelforce.svc.telephony.realtime"
	CurrentKeyID = "jwt-rsa-2048-v1"
)

type Claims struct {
	jwt.RegisteredClaims
}

func (claims *Claims) AccountID() string {
	if claims == nil || len(claims.Audience) == 0 {
		return ""
	}
	return claims.Audience[0]
}

type Validator struct {
	publicKey *rsa.PublicKey
}

func NewValidatorPEM(publicKeyPEM []byte) (*Validator, error) {
	publicKey, err := jwt.ParseRSAPublicKeyFromPEM(publicKeyPEM)
	if err != nil {
		return nil, fmt.Errorf("parse babelforce RSA public key: %w", err)
	}
	return &Validator{publicKey: publicKey}, nil
}

func (validator *Validator) ValidateAuthorization(value string) (*Claims, error) {
	tokenValue, found := strings.CutPrefix(value, "Bearer ")
	if !found || tokenValue == "" || strings.ContainsAny(tokenValue, " \t\r\n") {
		return nil, fmt.Errorf("missing or malformed bearer token")
	}

	claims := new(Claims)
	token, err := jwt.ParseWithClaims(
		tokenValue,
		claims,
		func(token *jwt.Token) (any, error) {
			if token.Method.Alg() != jwt.SigningMethodRS256.Alg() {
				return nil, fmt.Errorf("unexpected signing algorithm %q", token.Method.Alg())
			}
			return validator.publicKey, nil
		},
		jwt.WithValidMethods([]string{jwt.SigningMethodRS256.Alg()}),
		jwt.WithIssuer(Issuer),
		jwt.WithSubject(Subject),
		jwt.WithExpirationRequired(),
		jwt.WithIssuedAt(),
		jwt.WithStrictDecoding(),
	)
	if err != nil {
		return nil, fmt.Errorf("validate babelforce JWT: %w", err)
	}
	if !token.Valid {
		return nil, fmt.Errorf("validate babelforce JWT: token is invalid")
	}
	return claims, nil
}

func (validator *Validator) AuthHandler(request *http.Request) error {
	_, err := validator.ValidateAuthorization(request.Header.Get("Authorization"))
	return err
}
