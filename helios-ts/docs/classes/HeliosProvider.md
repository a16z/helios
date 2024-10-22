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

[lib.ts:15](https://github.com/a16z/helios/blob/main/helios-ts/lib.ts#L15)

## Methods

### request()

> **request**(`req`): `Promise`\<`any`\>

#### Parameters

• **req**: `Request`

#### Returns

`Promise`\<`any`\>

#### Defined in

[lib.ts:43](https://github.com/a16z/helios/blob/main/helios-ts/lib.ts#L43)

***

### sync()

> **sync**(): `Promise`\<`void`\>

#### Returns

`Promise`\<`void`\>

#### Defined in

[lib.ts:35](https://github.com/a16z/helios/blob/main/helios-ts/lib.ts#L35)

***

### waitSynced()

> **waitSynced**(): `Promise`\<`void`\>

#### Returns

`Promise`\<`void`\>

#### Defined in

[lib.ts:39](https://github.com/a16z/helios/blob/main/helios-ts/lib.ts#L39)
