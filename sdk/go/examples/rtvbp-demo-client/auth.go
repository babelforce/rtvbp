package main

import (
	"crypto/rsa"
	"crypto/x509"
	"embed"
	"encoding/pem"
	"fmt"
	"time"

	"github.com/golang-jwt/jwt/v5"
	gonanoid "github.com/matoous/go-nanoid/v2"
)

const (
	authJWTKeyID    = "jwt-rsa-2048-v1"                       // authJWTKeyID Key ID for the JWT header
	authJWTIssuer   = "auth.babelforce.com"                   // authJWTIssuer is the Issuer of the token
	authJWTAudience = "demo-account-123"                      // authJWTAudience is the Account ID for audience claim
	authJWTSubject  = "com.babelforce.svc.telephony.realtime" // authJWTSubject is the Subject identifier for the application
)

//go:embed keys/*.pem
var keyFiles embed.FS

// generateJWT generates a JWT with RSA256 signing
func generateJWT() (string, error) {
	// Load private key
	privateKey, err := loadPrivateKey("keys/private_key.pem")
	if err != nil {
		return "", fmt.Errorf("failed to load private key: %w", err)
	}

	// Generate unique token ID
	tokenID, err := gonanoid.New()
	if err != nil {
		return "", fmt.Errorf("failed to generate token ID: %w", err)
	}

	// Create token with claims
	now := time.Now()
	token := jwt.NewWithClaims(jwt.SigningMethodRS256, jwt.MapClaims{
		"exp": now.Add(time.Hour).Unix(), // Expires in 1 hour
		"iat": now.Unix(),                // Issued at current time
		"iss": authJWTIssuer,             // Issued by
		"aud": authJWTAudience,           // Audience - account ID
		"sub": authJWTSubject,            // Subject - application identifier
		"jti": tokenID,                   // JWT ID - unique token identifier
	})

	// Set the Key ID in header
	token.Header["kid"] = authJWTKeyID

	// Sign the token
	tokenString, err := token.SignedString(privateKey)
	if err != nil {
		return "", fmt.Errorf("failed to sign token: %w", err)
	}

	return tokenString, nil
}

// loadPrivateKey loads RSA private key from PEM file
func loadPrivateKey(filename string) (*rsa.PrivateKey, error) {
	keyData, err := keyFiles.ReadFile(filename)
	if err != nil {
		return nil, fmt.Errorf("failed to read key file: %w", err)
	}

	block, _ := pem.Decode(keyData)
	if block == nil {
		return nil, fmt.Errorf("failed to decode PEM block")
	}

	// Try PKCS1 first, then PKCS8
	privateKey, err := x509.ParsePKCS1PrivateKey(block.Bytes)
	if err != nil {
		// Try PKCS8 format
		key, err2 := x509.ParsePKCS8PrivateKey(block.Bytes)
		if err2 != nil {
			return nil, fmt.Errorf("failed to parse private key (tried both PKCS1 and PKCS8): %w", err)
		}

		rsaKey, ok := key.(*rsa.PrivateKey)
		if !ok {
			return nil, fmt.Errorf("key is not RSA private key")
		}
		return rsaKey, nil
	}

	return privateKey, nil
}
