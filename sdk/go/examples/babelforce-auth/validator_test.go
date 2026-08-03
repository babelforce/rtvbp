package babelforceauth

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"encoding/pem"
	"net/http"
	"testing"
	"time"

	"github.com/babelforce/rtvbp/sdk/go/transport/ws"
	"github.com/golang-jwt/jwt/v5"
)

func TestValidatorEnforcesBabelforceJWTContract(t *testing.T) {
	privateKey, validator := testValidator(t)
	now := time.Now()
	valid := Claims{RegisteredClaims: jwt.RegisteredClaims{
		Issuer:    Issuer,
		Subject:   Subject,
		Audience:  jwt.ClaimStrings{"account-123"},
		ExpiresAt: jwt.NewNumericDate(now.Add(time.Hour)),
		IssuedAt:  jwt.NewNumericDate(now),
		ID:        "token-123",
	}}

	tests := []struct {
		name   string
		header func() string
	}{
		{name: "missing bearer", header: func() string { return "" }},
		{name: "wrong signature", header: func() string {
			other, err := rsa.GenerateKey(rand.Reader, 2048)
			if err != nil {
				t.Fatal(err)
			}
			return "Bearer " + sign(t, other, jwt.SigningMethodRS256, valid)
		}},
		{name: "wrong algorithm", header: func() string {
			token := jwt.NewWithClaims(jwt.SigningMethodHS256, valid)
			value, err := token.SignedString([]byte("not-an-rsa-key"))
			if err != nil {
				t.Fatal(err)
			}
			return "Bearer " + value
		}},
		{name: "wrong issuer", header: func() string {
			claims := valid
			claims.Issuer = "other.example"
			return "Bearer " + sign(t, privateKey, jwt.SigningMethodRS256, claims)
		}},
		{name: "wrong subject", header: func() string {
			claims := valid
			claims.Subject = "other-subject"
			return "Bearer " + sign(t, privateKey, jwt.SigningMethodRS256, claims)
		}},
		{name: "missing expiry", header: func() string {
			claims := valid
			claims.ExpiresAt = nil
			return "Bearer " + sign(t, privateKey, jwt.SigningMethodRS256, claims)
		}},
		{name: "expired", header: func() string {
			claims := valid
			claims.ExpiresAt = jwt.NewNumericDate(now.Add(-time.Hour))
			return "Bearer " + sign(t, privateKey, jwt.SigningMethodRS256, claims)
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if _, err := validator.ValidateAuthorization(test.header()); err == nil {
				t.Fatal("invalid authorization was accepted")
			}
		})
	}

	header := "Bearer " + sign(t, privateKey, jwt.SigningMethodRS256, valid)
	claims, err := validator.ValidateAuthorization(header)
	if err != nil {
		t.Fatal(err)
	}
	if claims.AccountID() != "account-123" {
		t.Fatalf("account = %q, want account-123", claims.AccountID())
	}
}

func TestAudienceIsContextRatherThanFixedAuthorizationBoundary(t *testing.T) {
	privateKey, validator := testValidator(t)
	claims := Claims{RegisteredClaims: jwt.RegisteredClaims{
		Issuer:    Issuer,
		Subject:   Subject,
		Audience:  jwt.ClaimStrings{"another-account"},
		ExpiresAt: jwt.NewNumericDate(time.Now().Add(time.Hour)),
	}}
	got, err := validator.ValidateAuthorization(
		"Bearer " + sign(t, privateKey, jwt.SigningMethodRS256, claims),
	)
	if err != nil {
		t.Fatal(err)
	}
	if got.AccountID() != "another-account" {
		t.Fatalf("account = %q, want another-account", got.AccountID())
	}
}

func TestAuthHandlerWiresIntoWebSocketServerConfig(t *testing.T) {
	privateKey, validator := testValidator(t)
	request, err := http.NewRequest(http.MethodGet, "https://example.test/rtvbp", nil)
	if err != nil {
		t.Fatal(err)
	}
	claims := Claims{RegisteredClaims: jwt.RegisteredClaims{
		Issuer:    Issuer,
		Subject:   Subject,
		ExpiresAt: jwt.NewNumericDate(time.Now().Add(time.Hour)),
	}}
	request.Header.Set(
		"Authorization",
		"Bearer "+sign(t, privateKey, jwt.SigningMethodRS256, claims),
	)
	config := ws.ServerConfig{AuthHandler: validator.AuthHandler}
	if err := config.AuthHandler(request); err != nil {
		t.Fatal(err)
	}
}

func testValidator(t *testing.T) (*rsa.PrivateKey, *Validator) {
	t.Helper()
	privateKey, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		t.Fatal(err)
	}
	publicDER, err := x509.MarshalPKIXPublicKey(&privateKey.PublicKey)
	if err != nil {
		t.Fatal(err)
	}
	publicPEM := pem.EncodeToMemory(&pem.Block{Type: "PUBLIC KEY", Bytes: publicDER})
	validator, err := NewValidatorPEM(publicPEM)
	if err != nil {
		t.Fatal(err)
	}
	return privateKey, validator
}

func sign(
	t *testing.T,
	privateKey *rsa.PrivateKey,
	method jwt.SigningMethod,
	claims Claims,
) string {
	t.Helper()
	token := jwt.NewWithClaims(method, claims)
	token.Header["kid"] = CurrentKeyID
	value, err := token.SignedString(privateKey)
	if err != nil {
		t.Fatal(err)
	}
	return value
}
