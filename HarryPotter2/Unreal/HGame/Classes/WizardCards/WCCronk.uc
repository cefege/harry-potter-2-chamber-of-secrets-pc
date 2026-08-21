//===============================================================================
// #42	Crispin Cronk
//===============================================================================

class  WCCronk extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardCronkTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Cronksmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardCronkBigTexture FILE=TEXTURES\menu\Folio\Cards\Cronkbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Crispin Cronk";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=42
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE090"
    skin(0)=Texture'HProps.skins.WizardCardCronkTex0'
	textureBig=Texture'WizCardCronkBigTexture'
	strDescriptionId="WizCard_0080"
}
