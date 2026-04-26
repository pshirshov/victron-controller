// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'
import {CoreState, CoreState_UEBACodec} from './CoreState'

export class CoresState implements BaboonGenerated {
    private readonly _cores: Array<CoreState>;
    private readonly _topo_order: Array<string>;

    constructor(cores: Array<CoreState>, topo_order: Array<string>) {
        this._cores = cores
        this._topo_order = topo_order
    }

    public get cores(): Array<CoreState> {
        return this._cores;
    }
    public get topo_order(): Array<string> {
        return this._topo_order;
    }

    public toJSON(): Record<string, unknown> {
        return {
            cores: this._cores,
            topo_order: this._topo_order
        };
    }

    public with(overrides: {cores?: Array<CoreState>; topo_order?: Array<string>}): CoresState {
        return new CoresState(
            'cores' in overrides ? overrides.cores! : this._cores,
            'topo_order' in overrides ? overrides.topo_order! : this._topo_order
        );
    }

    public static fromPlain(obj: {cores: Array<CoreState>; topo_order: Array<string>}): CoresState {
        return new CoresState(
            obj.cores,
            obj.topo_order
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return CoresState.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return CoresState.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#CoresState'
    public baboonTypeIdentifier() {
        return CoresState.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return CoresState.BaboonSameInVersions
    }
    public static binCodec(): CoresState_UEBACodec {
        return CoresState_UEBACodec.instance
    }
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class CoresState_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: CoresState, writer: BaboonBinWriter): unknown {
        if (this !== CoresState_UEBACodec.lazyInstance.value) {
          return CoresState_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeI32(buffer, Array.from(value.cores).length);
            for (const item of value.cores) {
                CoreState_UEBACodec.instance.encode(ctx, item, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeI32(buffer, Array.from(value.topo_order).length);
            for (const item of value.topo_order) {
                BinTools.writeString(buffer, item);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeI32(writer, Array.from(value.cores).length);
            for (const item of value.cores) {
                CoreState_UEBACodec.instance.encode(ctx, item, writer);
            }
            BinTools.writeI32(writer, Array.from(value.topo_order).length);
            for (const item of value.topo_order) {
                BinTools.writeString(writer, item);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): CoresState {
        if (this !== CoresState_UEBACodec .lazyInstance.value) {
            return CoresState_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 2; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const cores = Array.from({ length: BinTools.readI32(reader) }, () => CoreState_UEBACodec.instance.decode(ctx, reader));
        const topo_order = Array.from({ length: BinTools.readI32(reader) }, () => BinTools.readString(reader));
        return new CoresState(
            cores,
            topo_order,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return CoresState_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return CoresState_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#CoresState'
    public baboonTypeIdentifier() {
        return CoresState_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new CoresState_UEBACodec())
    public static get instance(): CoresState_UEBACodec {
        return CoresState_UEBACodec.lazyInstance.value
    }
}