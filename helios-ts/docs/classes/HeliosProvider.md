[**helios**](../README.md) • **Docs**

***

[helios](../globals.md) / HeliosProvider

# Class: HeliosProvider

## Constructors

### new HeliosProvider()

> **new HeliosProvider**(`config`, `kind`): [`HeliosProvider`](HeliosProvider.md)

#### Parameters

• **config**: [`Config`](../type-aliases/Config.md)

• **kind**: `"ethereum"` \| `"opstack"`

#### Returns

[`HeliosProvider`](HeliosProvider.md)

#### Defined in

lib.d.ts:4

## Methods

### request()

> **request**(`req`): `Promise`\<`any`\>

#### Parameters

• **req**: `Request`

#### Returns

`Promise`\<`any`\>

#### Defined in

lib.d.ts:7

***

### sync()

> **sync**(): `Promise`\<`void`\>

#### Returns

`Promise`\<`void`\>

#### Defined in

lib.d.ts:5

***

### waitSynced()

> **waitSynced**(): `Promise`\<`void`\>

#### Returns

`Promise`\<`void`\>

#### Defined in

lib.d.ts:6
