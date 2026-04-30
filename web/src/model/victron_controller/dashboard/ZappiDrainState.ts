// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {ZappiDrainSnapshotWire, ZappiDrainSnapshotWire_UEBACodec} from './ZappiDrainSnapshotWire'
import {ZappiDrainSample, ZappiDrainSample_UEBACodec} from './ZappiDrainSample'

export class ZappiDrainState implements BaboonGeneratedLatest {
    private readonly _latest: ZappiDrainSnapshotWire | undefined;
    private readonly _samples: Array<ZappiDrainSample>;

    constructor(latest: ZappiDrainSnapshotWire | undefined, samples: Array<ZappiDrainSample>) {
        this._latest = latest
        this._samples = samples
    }

    public get latest(): ZappiDrainSnapshotWire | undefined {
        return this._latest;
    }
    public get samples(): Array<ZappiDrainSample> {
        return this._samples;
    }

    public toJSON(): Record<string, unknown> {
        return {
            latest: this._latest !== undefined ? this._latest : undefined,
            samples: this._samples
        };
    }

    public with(overrides: {latest?: ZappiDrainSnapshotWire | undefined; samples?: Array<ZappiDrainSample>}): ZappiDrainState {
        return new ZappiDrainState(
            'latest' in overrides ? overrides.latest! : this._latest,
            'samples' in overrides ? overrides.samples! : this._samples
        );
    }

    public static fromPlain(obj: {latest: ZappiDrainSnapshotWire | undefined; samples: Array<ZappiDrainSample>}): ZappiDrainState {
        return new ZappiDrainState(
            obj.latest,
            obj.samples
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ZappiDrainState.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ZappiDrainState.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ZappiDrainState'
    public baboonTypeIdentifier() {
        return ZappiDrainState.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return ZappiDrainState.BaboonSameInVersions
    }
    public static binCodec(): ZappiDrainState_UEBACodec {
        return ZappiDrainState_UEBACodec.instance
    }
}

export class ZappiDrainState_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: ZappiDrainState, writer: BaboonBinWriter): unknown {
        if (this !== ZappiDrainState_UEBACodec.lazyInstance.value) {
          return ZappiDrainState_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.latest === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                ZappiDrainSnapshotWire_UEBACodec.instance.encode(ctx, value.latest, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeI32(buffer, Array.from(value.samples).length);
            for (const item of value.samples) {
                ZappiDrainSample_UEBACodec.instance.encode(ctx, item, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            if (value.latest === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                ZappiDrainSnapshotWire_UEBACodec.instance.encode(ctx, value.latest, writer);
            }
            BinTools.writeI32(writer, Array.from(value.samples).length);
            for (const item of value.samples) {
                ZappiDrainSample_UEBACodec.instance.encode(ctx, item, writer);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ZappiDrainState {
        if (this !== ZappiDrainState_UEBACodec .lazyInstance.value) {
            return ZappiDrainState_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 2; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const latest = (BinTools.readByte(reader) === 0 ? undefined : ZappiDrainSnapshotWire_UEBACodec.instance.decode(ctx, reader));
        const samples = Array.from({ length: BinTools.readI32(reader) }, () => ZappiDrainSample_UEBACodec.instance.decode(ctx, reader));
        return new ZappiDrainState(
            latest,
            samples,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ZappiDrainState_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ZappiDrainState_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ZappiDrainState'
    public baboonTypeIdentifier() {
        return ZappiDrainState_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new ZappiDrainState_UEBACodec())
    public static get instance(): ZappiDrainState_UEBACodec {
        return ZappiDrainState_UEBACodec.lazyInstance.value
    }
}