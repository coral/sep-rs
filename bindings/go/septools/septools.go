package septools

// #include <sep_tools.h>
import "C"

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"io"
	"math"
	"unsafe"
)

// This is needed, because as of go 1.24
// type RustBuffer C.RustBuffer cannot have methods,
// RustBuffer is treated as non-local type
type GoRustBuffer struct {
	inner C.RustBuffer
}

type RustBufferI interface {
	AsReader() *bytes.Reader
	Free()
	ToGoBytes() []byte
	Data() unsafe.Pointer
	Len() uint64
	Capacity() uint64
}

// C.RustBuffer fields exposed as an interface so they can be accessed in different Go packages.
// See https://github.com/golang/go/issues/13467
type ExternalCRustBuffer interface {
	Data() unsafe.Pointer
	Len() uint64
	Capacity() uint64
}

func RustBufferFromC(b C.RustBuffer) ExternalCRustBuffer {
	return GoRustBuffer{
		inner: b,
	}
}

func CFromRustBuffer(b ExternalCRustBuffer) C.RustBuffer {
	return C.RustBuffer{
		capacity: C.uint64_t(b.Capacity()),
		len:      C.uint64_t(b.Len()),
		data:     (*C.uchar)(b.Data()),
	}
}

func RustBufferFromExternal(b ExternalCRustBuffer) GoRustBuffer {
	return GoRustBuffer{
		inner: C.RustBuffer{
			capacity: C.uint64_t(b.Capacity()),
			len:      C.uint64_t(b.Len()),
			data:     (*C.uchar)(b.Data()),
		},
	}
}

func (cb GoRustBuffer) Capacity() uint64 {
	return uint64(cb.inner.capacity)
}

func (cb GoRustBuffer) Len() uint64 {
	return uint64(cb.inner.len)
}

func (cb GoRustBuffer) Data() unsafe.Pointer {
	return unsafe.Pointer(cb.inner.data)
}

func (cb GoRustBuffer) AsReader() *bytes.Reader {
	b := unsafe.Slice((*byte)(cb.inner.data), C.uint64_t(cb.inner.len))
	return bytes.NewReader(b)
}

func (cb GoRustBuffer) Free() {
	rustCall(func(status *C.RustCallStatus) bool {
		C.ffi_sep_rs_ffi_rustbuffer_free(cb.inner, status)
		return false
	})
}

func (cb GoRustBuffer) ToGoBytes() []byte {
	return C.GoBytes(unsafe.Pointer(cb.inner.data), C.int(cb.inner.len))
}

func stringToRustBuffer(str string) C.RustBuffer {
	return bytesToRustBuffer([]byte(str))
}

func bytesToRustBuffer(b []byte) C.RustBuffer {
	if len(b) == 0 {
		return C.RustBuffer{}
	}
	// We can pass the pointer along here, as it is pinned
	// for the duration of this call
	foreign := C.ForeignBytes{
		len:  C.int(len(b)),
		data: (*C.uchar)(unsafe.Pointer(&b[0])),
	}

	return rustCall(func(status *C.RustCallStatus) C.RustBuffer {
		return C.ffi_sep_rs_ffi_rustbuffer_from_bytes(foreign, status)
	})
}

type BufLifter[GoType any] interface {
	Lift(value RustBufferI) GoType
}

type BufLowerer[GoType any] interface {
	Lower(value GoType) C.RustBuffer
}

type BufReader[GoType any] interface {
	Read(reader io.Reader) GoType
}

type BufWriter[GoType any] interface {
	Write(writer io.Writer, value GoType)
}

func LowerIntoRustBuffer[GoType any](bufWriter BufWriter[GoType], value GoType) C.RustBuffer {
	// This might be not the most efficient way but it does not require knowing allocation size
	// beforehand
	var buffer bytes.Buffer
	bufWriter.Write(&buffer, value)

	bytes, err := io.ReadAll(&buffer)
	if err != nil {
		panic(fmt.Errorf("reading written data: %w", err))
	}
	return bytesToRustBuffer(bytes)
}

func LiftFromRustBuffer[GoType any](bufReader BufReader[GoType], rbuf RustBufferI) GoType {
	defer rbuf.Free()
	reader := rbuf.AsReader()
	item := bufReader.Read(reader)
	if reader.Len() > 0 {
		// TODO: Remove this
		leftover, _ := io.ReadAll(reader)
		panic(fmt.Errorf("Junk remaining in buffer after lifting: %s", string(leftover)))
	}
	return item
}

func rustCallWithError[E any, U any](converter BufReader[E], callback func(*C.RustCallStatus) U) (U, E) {
	var status C.RustCallStatus
	returnValue := callback(&status)
	err := checkCallStatus(converter, status)
	return returnValue, err
}

func checkCallStatus[E any](converter BufReader[E], status C.RustCallStatus) E {
	switch status.code {
	case 0:
		var zero E
		return zero
	case 1:
		return LiftFromRustBuffer(converter, GoRustBuffer{inner: status.errorBuf})
	case 2:
		// when the rust code sees a panic, it tries to construct a rustBuffer
		// with the message.  but if that code panics, then it just sends back
		// an empty buffer.
		if status.errorBuf.len > 0 {
			panic(fmt.Errorf("%s", FfiConverterStringINSTANCE.Lift(GoRustBuffer{inner: status.errorBuf})))
		} else {
			panic(fmt.Errorf("Rust panicked while handling Rust panic"))
		}
	default:
		panic(fmt.Errorf("unknown status code: %d", status.code))
	}
}

func checkCallStatusUnknown(status C.RustCallStatus) error {
	switch status.code {
	case 0:
		return nil
	case 1:
		panic(fmt.Errorf("function not returning an error returned an error"))
	case 2:
		// when the rust code sees a panic, it tries to construct a C.RustBuffer
		// with the message.  but if that code panics, then it just sends back
		// an empty buffer.
		if status.errorBuf.len > 0 {
			panic(fmt.Errorf("%s", FfiConverterStringINSTANCE.Lift(GoRustBuffer{
				inner: status.errorBuf,
			})))
		} else {
			panic(fmt.Errorf("Rust panicked while handling Rust panic"))
		}
	default:
		return fmt.Errorf("unknown status code: %d", status.code)
	}
}

func rustCall[U any](callback func(*C.RustCallStatus) U) U {
	returnValue, err := rustCallWithError[error](nil, callback)
	if err != nil {
		panic(err)
	}
	return returnValue
}

type NativeError interface {
	AsError() error
}

func writeInt8(writer io.Writer, value int8) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeUint8(writer io.Writer, value uint8) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeInt16(writer io.Writer, value int16) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeUint16(writer io.Writer, value uint16) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeInt32(writer io.Writer, value int32) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeUint32(writer io.Writer, value uint32) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeInt64(writer io.Writer, value int64) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeUint64(writer io.Writer, value uint64) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeFloat32(writer io.Writer, value float32) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func writeFloat64(writer io.Writer, value float64) {
	if err := binary.Write(writer, binary.BigEndian, value); err != nil {
		panic(err)
	}
}

func readInt8(reader io.Reader) int8 {
	var result int8
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readUint8(reader io.Reader) uint8 {
	var result uint8
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readInt16(reader io.Reader) int16 {
	var result int16
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readUint16(reader io.Reader) uint16 {
	var result uint16
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readInt32(reader io.Reader) int32 {
	var result int32
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readUint32(reader io.Reader) uint32 {
	var result uint32
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readInt64(reader io.Reader) int64 {
	var result int64
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readUint64(reader io.Reader) uint64 {
	var result uint64
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readFloat32(reader io.Reader) float32 {
	var result float32
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func readFloat64(reader io.Reader) float64 {
	var result float64
	if err := binary.Read(reader, binary.BigEndian, &result); err != nil {
		panic(err)
	}
	return result
}

func init() {

	uniffiCheckChecksums()
}

func uniffiCheckChecksums() {
	// Get the bindings contract version from our ComponentInterface
	bindingsContractVersion := 30
	// Get the scaffolding contract version by calling the into the dylib
	scaffoldingContractVersion := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint32_t {
		return C.ffi_sep_rs_ffi_uniffi_contract_version()
	})
	if bindingsContractVersion != int(scaffoldingContractVersion) {
		// If this happens try cleaning and rebuilding your project
		panic("septools: UniFFI contract version mismatch")
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_sep_rs_ffi_checksum_func_generate_bundle_json()
		})
		if checksum != 59412 {
			// If this happens try cleaning and rebuilding your project
			panic("septools: uniffi_sep_rs_ffi_checksum_func_generate_bundle_json: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_sep_rs_ffi_checksum_func_generate_device_json()
		})
		if checksum != 22643 {
			// If this happens try cleaning and rebuilding your project
			panic("septools: uniffi_sep_rs_ffi_checksum_func_generate_device_json: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_sep_rs_ffi_checksum_func_model_profiles_json()
		})
		if checksum != 14017 {
			// If this happens try cleaning and rebuilding your project
			panic("septools: uniffi_sep_rs_ffi_checksum_func_model_profiles_json: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_sep_rs_ffi_checksum_func_options_json()
		})
		if checksum != 56213 {
			// If this happens try cleaning and rebuilding your project
			panic("septools: uniffi_sep_rs_ffi_checksum_func_options_json: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_sep_rs_ffi_checksum_func_phone_options_json()
		})
		if checksum != 56889 {
			// If this happens try cleaning and rebuilding your project
			panic("septools: uniffi_sep_rs_ffi_checksum_func_phone_options_json: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_sep_rs_ffi_checksum_func_validate_artifact_json()
		})
		if checksum != 51074 {
			// If this happens try cleaning and rebuilding your project
			panic("septools: uniffi_sep_rs_ffi_checksum_func_validate_artifact_json: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_sep_rs_ffi_checksum_func_validate_bundle_json()
		})
		if checksum != 13307 {
			// If this happens try cleaning and rebuilding your project
			panic("septools: uniffi_sep_rs_ffi_checksum_func_validate_bundle_json: UniFFI API checksum mismatch")
		}
	}
	{
		checksum := rustCall(func(_uniffiStatus *C.RustCallStatus) C.uint16_t {
			return C.uniffi_sep_rs_ffi_checksum_func_validate_phone_settings_json()
		})
		if checksum != 29374 {
			// If this happens try cleaning and rebuilding your project
			panic("septools: uniffi_sep_rs_ffi_checksum_func_validate_phone_settings_json: UniFFI API checksum mismatch")
		}
	}
}

type FfiConverterString struct{}

var FfiConverterStringINSTANCE = FfiConverterString{}

func (FfiConverterString) Lift(rb RustBufferI) string {
	defer rb.Free()
	reader := rb.AsReader()
	b, err := io.ReadAll(reader)
	if err != nil {
		panic(fmt.Errorf("reading reader: %w", err))
	}
	return string(b)
}

func (FfiConverterString) Read(reader io.Reader) string {
	length := readInt32(reader)
	buffer := make([]byte, length)
	read_length, err := reader.Read(buffer)
	if err != nil && err != io.EOF {
		panic(err)
	}
	if read_length != int(length) {
		panic(fmt.Errorf("bad read length when reading string, expected %d, read %d", length, read_length))
	}
	return string(buffer)
}

func (FfiConverterString) Lower(value string) C.RustBuffer {
	return stringToRustBuffer(value)
}

func (c FfiConverterString) LowerExternal(value string) ExternalCRustBuffer {
	return RustBufferFromC(stringToRustBuffer(value))
}

func (FfiConverterString) Write(writer io.Writer, value string) {
	if len(value) > math.MaxInt32 {
		panic("String is too large to fit into Int32")
	}

	writeInt32(writer, int32(len(value)))
	write_length, err := io.WriteString(writer, value)
	if err != nil {
		panic(err)
	}
	if write_length != len(value) {
		panic(fmt.Errorf("bad write length when writing string, expected %d, written %d", len(value), write_length))
	}
}

type FfiDestroyerString struct{}

func (FfiDestroyerString) Destroy(_ string) {}

type SepToolsError struct {
	err error
}

// Convenience method to turn *SepToolsError into error
// Avoiding treating nil pointer as non nil error interface
func (err *SepToolsError) AsError() error {
	if err == nil {
		return nil
	} else {
		return err
	}
}

func (err SepToolsError) Error() string {
	return fmt.Sprintf("SepToolsError: %s", err.err.Error())
}

func (err SepToolsError) Unwrap() error {
	return err.err
}

// Err* are used for checking error type with `errors.Is`
var ErrSepToolsErrorInvalidRequest = fmt.Errorf("SepToolsErrorInvalidRequest")
var ErrSepToolsErrorOperationFailed = fmt.Errorf("SepToolsErrorOperationFailed")

// Variant structs
type SepToolsErrorInvalidRequest struct {
	Message string
}

func NewSepToolsErrorInvalidRequest(
	message string,
) *SepToolsError {
	return &SepToolsError{err: &SepToolsErrorInvalidRequest{
		Message: message}}
}

func (e SepToolsErrorInvalidRequest) destroy() {
	FfiDestroyerString{}.Destroy(e.Message)
}

func (err SepToolsErrorInvalidRequest) Error() string {
	return fmt.Sprint("InvalidRequest",
		": ",

		"Message=",
		err.Message,
	)
}

func (self SepToolsErrorInvalidRequest) Is(target error) bool {
	return target == ErrSepToolsErrorInvalidRequest
}

type SepToolsErrorOperationFailed struct {
	Message string
}

func NewSepToolsErrorOperationFailed(
	message string,
) *SepToolsError {
	return &SepToolsError{err: &SepToolsErrorOperationFailed{
		Message: message}}
}

func (e SepToolsErrorOperationFailed) destroy() {
	FfiDestroyerString{}.Destroy(e.Message)
}

func (err SepToolsErrorOperationFailed) Error() string {
	return fmt.Sprint("OperationFailed",
		": ",

		"Message=",
		err.Message,
	)
}

func (self SepToolsErrorOperationFailed) Is(target error) bool {
	return target == ErrSepToolsErrorOperationFailed
}

type FfiConverterSepToolsError struct{}

var FfiConverterSepToolsErrorINSTANCE = FfiConverterSepToolsError{}

func (c FfiConverterSepToolsError) Lift(eb RustBufferI) *SepToolsError {
	return LiftFromRustBuffer[*SepToolsError](c, eb)
}

func (c FfiConverterSepToolsError) Lower(value *SepToolsError) C.RustBuffer {
	return LowerIntoRustBuffer[*SepToolsError](c, value)
}

func (c FfiConverterSepToolsError) LowerExternal(value *SepToolsError) ExternalCRustBuffer {
	return RustBufferFromC(LowerIntoRustBuffer[*SepToolsError](c, value))
}

func (c FfiConverterSepToolsError) Read(reader io.Reader) *SepToolsError {
	errorID := readUint32(reader)

	switch errorID {
	case 1:
		return &SepToolsError{&SepToolsErrorInvalidRequest{
			Message: FfiConverterStringINSTANCE.Read(reader),
		}}
	case 2:
		return &SepToolsError{&SepToolsErrorOperationFailed{
			Message: FfiConverterStringINSTANCE.Read(reader),
		}}
	default:
		panic(fmt.Sprintf("Unknown error code %d in FfiConverterSepToolsError.Read()", errorID))
	}
}

func (c FfiConverterSepToolsError) Write(writer io.Writer, value *SepToolsError) {
	switch variantValue := value.err.(type) {
	case *SepToolsErrorInvalidRequest:
		writeInt32(writer, 1)
		FfiConverterStringINSTANCE.Write(writer, variantValue.Message)
	case *SepToolsErrorOperationFailed:
		writeInt32(writer, 2)
		FfiConverterStringINSTANCE.Write(writer, variantValue.Message)
	default:
		_ = variantValue
		panic(fmt.Sprintf("invalid error value `%v` in FfiConverterSepToolsError.Write", value))
	}
}

type FfiDestroyerSepToolsError struct{}

func (_ FfiDestroyerSepToolsError) Destroy(value *SepToolsError) {
	switch variantValue := value.err.(type) {
	case SepToolsErrorInvalidRequest:
		variantValue.destroy()
	case SepToolsErrorOperationFailed:
		variantValue.destroy()
	default:
		_ = variantValue
		panic(fmt.Sprintf("invalid error value `%v` in FfiDestroyerSepToolsError.Destroy", value))
	}
}

type FfiConverterOptionalString struct{}

var FfiConverterOptionalStringINSTANCE = FfiConverterOptionalString{}

func (c FfiConverterOptionalString) Lift(rb RustBufferI) *string {
	return LiftFromRustBuffer[*string](c, rb)
}

func (_ FfiConverterOptionalString) Read(reader io.Reader) *string {
	if readInt8(reader) == 0 {
		return nil
	}
	temp := FfiConverterStringINSTANCE.Read(reader)
	return &temp
}

func (c FfiConverterOptionalString) Lower(value *string) C.RustBuffer {
	return LowerIntoRustBuffer[*string](c, value)
}

func (c FfiConverterOptionalString) LowerExternal(value *string) ExternalCRustBuffer {
	return RustBufferFromC(LowerIntoRustBuffer[*string](c, value))
}

func (_ FfiConverterOptionalString) Write(writer io.Writer, value *string) {
	if value == nil {
		writeInt8(writer, 0)
	} else {
		writeInt8(writer, 1)
		FfiConverterStringINSTANCE.Write(writer, *value)
	}
}

type FfiDestroyerOptionalString struct{}

func (_ FfiDestroyerOptionalString) Destroy(value *string) {
	if value != nil {
		FfiDestroyerString{}.Destroy(*value)
	}
}

func GenerateBundleJson(requestJson string) (string, error) {
	_uniffiRV, _uniffiErr := rustCallWithError[*SepToolsError](FfiConverterSepToolsError{}, func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_sep_rs_ffi_fn_func_generate_bundle_json(FfiConverterStringINSTANCE.Lower(requestJson), _uniffiStatus),
		}
	})
	if _uniffiErr != nil {
		var _uniffiDefaultValue string
		return _uniffiDefaultValue, _uniffiErr
	} else {
		return FfiConverterStringINSTANCE.Lift(_uniffiRV), nil
	}
}

func GenerateDeviceJson(requestJson string) (string, error) {
	_uniffiRV, _uniffiErr := rustCallWithError[*SepToolsError](FfiConverterSepToolsError{}, func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_sep_rs_ffi_fn_func_generate_device_json(FfiConverterStringINSTANCE.Lower(requestJson), _uniffiStatus),
		}
	})
	if _uniffiErr != nil {
		var _uniffiDefaultValue string
		return _uniffiDefaultValue, _uniffiErr
	} else {
		return FfiConverterStringINSTANCE.Lift(_uniffiRV), nil
	}
}

func ModelProfilesJson() (string, error) {
	_uniffiRV, _uniffiErr := rustCallWithError[*SepToolsError](FfiConverterSepToolsError{}, func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_sep_rs_ffi_fn_func_model_profiles_json(_uniffiStatus),
		}
	})
	if _uniffiErr != nil {
		var _uniffiDefaultValue string
		return _uniffiDefaultValue, _uniffiErr
	} else {
		return FfiConverterStringINSTANCE.Lift(_uniffiRV), nil
	}
}

func OptionsJson(target *string) (string, error) {
	_uniffiRV, _uniffiErr := rustCallWithError[*SepToolsError](FfiConverterSepToolsError{}, func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_sep_rs_ffi_fn_func_options_json(FfiConverterOptionalStringINSTANCE.Lower(target), _uniffiStatus),
		}
	})
	if _uniffiErr != nil {
		var _uniffiDefaultValue string
		return _uniffiDefaultValue, _uniffiErr
	} else {
		return FfiConverterStringINSTANCE.Lift(_uniffiRV), nil
	}
}

func PhoneOptionsJson(model string, protocol string) (string, error) {
	_uniffiRV, _uniffiErr := rustCallWithError[*SepToolsError](FfiConverterSepToolsError{}, func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_sep_rs_ffi_fn_func_phone_options_json(FfiConverterStringINSTANCE.Lower(model), FfiConverterStringINSTANCE.Lower(protocol), _uniffiStatus),
		}
	})
	if _uniffiErr != nil {
		var _uniffiDefaultValue string
		return _uniffiDefaultValue, _uniffiErr
	} else {
		return FfiConverterStringINSTANCE.Lift(_uniffiRV), nil
	}
}

func ValidateArtifactJson(requestJson string) (string, error) {
	_uniffiRV, _uniffiErr := rustCallWithError[*SepToolsError](FfiConverterSepToolsError{}, func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_sep_rs_ffi_fn_func_validate_artifact_json(FfiConverterStringINSTANCE.Lower(requestJson), _uniffiStatus),
		}
	})
	if _uniffiErr != nil {
		var _uniffiDefaultValue string
		return _uniffiDefaultValue, _uniffiErr
	} else {
		return FfiConverterStringINSTANCE.Lift(_uniffiRV), nil
	}
}

func ValidateBundleJson(requestJson string) (string, error) {
	_uniffiRV, _uniffiErr := rustCallWithError[*SepToolsError](FfiConverterSepToolsError{}, func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_sep_rs_ffi_fn_func_validate_bundle_json(FfiConverterStringINSTANCE.Lower(requestJson), _uniffiStatus),
		}
	})
	if _uniffiErr != nil {
		var _uniffiDefaultValue string
		return _uniffiDefaultValue, _uniffiErr
	} else {
		return FfiConverterStringINSTANCE.Lift(_uniffiRV), nil
	}
}

func ValidatePhoneSettingsJson(requestJson string) (string, error) {
	_uniffiRV, _uniffiErr := rustCallWithError[*SepToolsError](FfiConverterSepToolsError{}, func(_uniffiStatus *C.RustCallStatus) RustBufferI {
		return GoRustBuffer{
			inner: C.uniffi_sep_rs_ffi_fn_func_validate_phone_settings_json(FfiConverterStringINSTANCE.Lower(requestJson), _uniffiStatus),
		}
	})
	if _uniffiErr != nil {
		var _uniffiDefaultValue string
		return _uniffiDefaultValue, _uniffiErr
	} else {
		return FfiConverterStringINSTANCE.Lift(_uniffiRV), nil
	}
}
