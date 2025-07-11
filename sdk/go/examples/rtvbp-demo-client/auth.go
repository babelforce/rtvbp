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
	// Key ID for the JWT header
	keyID = "dev-rtvbp-demo-key-1"
	// Account ID for audience claim
	accountID = "demo-account-123"
	// Subject identifier for the application
	subject = "rtvbp.auth.babelforce.com"
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
		"aud": accountID,                 // Audience - account ID
		"sub": subject,                   // Subject - application identifier
		"jti": tokenID,                   // JWT ID - unique token identifier
	})

	// Set the Key ID in header
	token.Header["kid"] = keyID

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

// loadPublicKey loads RSA public key from PEM file
func loadPublicKey(filename string) (*rsa.PublicKey, error) {
	keyData, err := keyFiles.ReadFile(filename)
	if err != nil {
		return nil, fmt.Errorf("failed to read key file: %w", err)
	}

	block, _ := pem.Decode(keyData)
	if block == nil {
		return nil, fmt.Errorf("failed to decode PEM block")
	}

	publicKey, err := x509.ParsePKIXPublicKey(block.Bytes)
	if err != nil {
		return nil, fmt.Errorf("failed to parse public key: %w", err)
	}

	rsaPublicKey, ok := publicKey.(*rsa.PublicKey)
	if !ok {
		return nil, fmt.Errorf("key is not RSA public key")
	}

	return rsaPublicKey, nil
}
